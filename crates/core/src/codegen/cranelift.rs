use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::entities::GlobalValue;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::ir::{Block, FuncRef, InstBuilder, Value};
use cranelift_codegen::isa::OwnedTargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::Variable;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};
use tempfile::Builder as TempDirBuilder;
use which::which;

use crate::diagnostics::{LangError, LangResult};
use crate::mir::{
    MirBinaryOp, MirExpr, MirExprKind, MirProgram, MirStatement, MirType, MirUnaryOp, VarId,
};

use super::BuildProfile;

const OBJECT_FILE_NAME: &str = "program.o";

pub struct CraneliftBackend {
    profile: BuildProfile,
}

impl CraneliftBackend {
    pub fn new(profile: BuildProfile) -> LangResult<Self> {
        Ok(Self { profile })
    }

    pub fn compile(&mut self, program: &MirProgram, output: &Path) -> LangResult<()> {
        let isa = self.create_isa()?;
        let mut module = self.create_module(isa)?;
        let declared = DeclaredSymbols::declare(&mut module)?;
        let object = self.emit_object(module, &declared, program)?;
        let tempdir = TempDirBuilder::new()
            .prefix("lang-native-")
            .tempdir()
            .map_err(|err| LangError::Runtime(format!("failed to create tempdir: {err}")))?;
        let object_path = tempdir.path().join(OBJECT_FILE_NAME);
        self.write_object(object, &object_path)?;
        self.invoke_linker(&object_path, output)?;
        Ok(())
    }

    fn create_isa(&self) -> LangResult<OwnedTargetIsa> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("opt_level", self.profile.opt_level())
            .map_err(|err| {
                LangError::Runtime(format!("failed to configure codegen settings: {err}"))
            })?;
        let isa_builder = cranelift_native::builder()
            .map_err(|err| LangError::Runtime(format!("native ISA not supported: {err}")))?;
        isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|err| LangError::Runtime(format!("failed to build ISA: {err}")))
    }

    fn create_module(&self, isa: OwnedTargetIsa) -> LangResult<ObjectModule> {
        let builder = ObjectBuilder::new(
            isa,
            "lang_program",
            cranelift_module::default_libcall_names(),
        )
        .map_err(|err| LangError::Runtime(format!("failed to construct object module: {err}")))?;
        Ok(ObjectModule::new(builder))
    }

    fn emit_object(
        &self,
        mut module: ObjectModule,
        declared: &DeclaredSymbols,
        program: &MirProgram,
    ) -> LangResult<ObjectProduct> {
        let mut ctx = module.make_context();
        ctx.func.signature.returns.push(AbiParam::new(types::I32));
        let main_id = module
            .declare_function("main", Linkage::Export, &ctx.func.signature)
            .map_err(|err| LangError::Runtime(format!("failed to declare main: {err}")))?;
        let mut builder_ctx = cranelift_frontend::FunctionBuilderContext::new();
        {
            let mut builder =
                cranelift_frontend::FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let printf_ref = module.declare_func_in_func(declared.printf, builder.func);
            let puts_ref = module.declare_func_in_func(declared.puts, builder.func);
            let mut compiler = FunctionCompiler::new(
                &mut module,
                builder,
                declared,
                printf_ref,
                puts_ref,
                program,
            );
            compiler.emit_program(program)?;
            let zero = compiler.builder.ins().iconst(types::I32, 0);
            compiler.builder.ins().return_(&[zero]);
            compiler.finish();
        }

        module
            .define_function(main_id, &mut ctx)
            .map_err(|err| LangError::Runtime(format!("failed to define main: {err}")))?;
        module.clear_context(&mut ctx);
        Ok(module.finish())
    }

    fn write_object(&self, product: ObjectProduct, path: &Path) -> LangResult<()> {
        let bytes = product
            .emit()
            .map_err(|err| LangError::Runtime(format!("failed to emit object bytes: {err}")))?;
        let mut file = File::create(path)
            .map_err(|err| LangError::Runtime(format!("failed to create object file: {err}")))?;
        file.write_all(&bytes)
            .map_err(|err| LangError::Runtime(format!("failed to write object file: {err}")))
    }

    fn invoke_linker(&self, object: &Path, output: &Path) -> LangResult<()> {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    LangError::Runtime(format!(
                        "failed to create output directory {}: {err}",
                        parent.display()
                    ))
                })?;
            }
        }
        let linker = find_linker().ok_or_else(|| {
            LangError::Runtime(
                "failed to locate system linker (`cc`, `clang`, or `gcc`) for native build".into(),
            )
        })?;
        let mut command = Command::new(&linker);
        command.arg(object).arg("-o").arg(output);
        let status = command.status().map_err(|err| {
            LangError::Runtime(format!("failed to invoke linker `{linker}`: {err}"))
        })?;
        if !status.success() {
            return Err(LangError::Runtime(format!(
                "linker `{linker}` exited with status {status}"
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(output)
                .map_err(|err| LangError::Runtime(format!("failed to read permissions: {err}")))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(output, perms).map_err(|err| {
                LangError::Runtime(format!(
                    "failed to set permissions on {}: {err}",
                    output.display()
                ))
            })?;
        }
        Ok(())
    }
}

struct DeclaredSymbols {
    printf: FuncId,
    puts: FuncId,
    int_format: DataId,
    true_str: DataId,
    false_str: DataId,
}

impl DeclaredSymbols {
    fn declare(module: &mut ObjectModule) -> LangResult<Self> {
        let pointer_type = module.isa().pointer_type();

        let mut printf_sig = module.make_signature();
        printf_sig.params.push(AbiParam::new(pointer_type));
        printf_sig.params.push(AbiParam::new(types::I64));
        printf_sig.returns.push(AbiParam::new(types::I32));
        let printf = module
            .declare_function("printf", Linkage::Import, &printf_sig)
            .map_err(|err| LangError::Runtime(format!("failed to declare printf: {err}")))?;

        let mut puts_sig = module.make_signature();
        puts_sig.params.push(AbiParam::new(pointer_type));
        puts_sig.returns.push(AbiParam::new(types::I32));
        let puts = module
            .declare_function("puts", Linkage::Import, &puts_sig)
            .map_err(|err| LangError::Runtime(format!("failed to declare puts: {err}")))?;

        let mut format_data = DataDescription::new();
        format_data.define(b"%lld\n\0".to_vec().into_boxed_slice());
        let int_format = module
            .declare_data("lang.int.format", Linkage::Local, false, false)
            .map_err(|err| LangError::Runtime(format!("failed to declare format string: {err}")))?;
        module
            .define_data(int_format, &format_data)
            .map_err(|err| LangError::Runtime(format!("failed to define format string: {err}")))?;

        let mut true_data = DataDescription::new();
        true_data.define(b"true\0".to_vec().into_boxed_slice());
        let true_str = module
            .declare_data("lang.bool.true", Linkage::Local, false, false)
            .map_err(|err| LangError::Runtime(format!("failed to declare true literal: {err}")))?;
        module
            .define_data(true_str, &true_data)
            .map_err(|err| LangError::Runtime(format!("failed to define true literal: {err}")))?;

        let mut false_data = DataDescription::new();
        false_data.define(b"false\0".to_vec().into_boxed_slice());
        let false_str = module
            .declare_data("lang.bool.false", Linkage::Local, false, false)
            .map_err(|err| LangError::Runtime(format!("failed to declare false literal: {err}")))?;
        module
            .define_data(false_str, &false_data)
            .map_err(|err| LangError::Runtime(format!("failed to define false literal: {err}")))?;

        Ok(Self {
            printf,
            puts,
            int_format,
            true_str,
            false_str,
        })
    }
}

struct FunctionCompiler<'a> {
    module: &'a mut ObjectModule,
    declared: &'a DeclaredSymbols,
    pub builder: cranelift_frontend::FunctionBuilder<'a>,
    printf_ref: FuncRef,
    puts_ref: FuncRef,
    pointer_type: cranelift_codegen::ir::Type,
    variable_defs: Vec<Option<VariableInfo>>,
    loop_stack: Vec<LoopContext>,
    block_terminated: bool,
    int_format_global: Option<GlobalValue>,
    true_global: Option<GlobalValue>,
    false_global: Option<GlobalValue>,
    variable_types: Vec<MirType>,
}

#[derive(Clone, Copy)]
struct VariableInfo {
    variable: Variable,
    ty: MirType,
}

struct LoopContext {
    break_block: Block,
    continue_block: Block,
}

impl<'a> FunctionCompiler<'a> {
    fn new(
        module: &'a mut ObjectModule,
        builder: cranelift_frontend::FunctionBuilder<'a>,
        declared: &'a DeclaredSymbols,
        printf_ref: FuncRef,
        puts_ref: FuncRef,
        program: &MirProgram,
    ) -> Self {
        let pointer_type = module.isa().pointer_type();
        Self {
            module,
            declared,
            builder,
            printf_ref,
            puts_ref,
            pointer_type,
            variable_defs: vec![None; program.variables.len()],
            loop_stack: Vec::new(),
            block_terminated: false,
            int_format_global: None,
            true_global: None,
            false_global: None,
            variable_types: program.variables.iter().map(|var| var.ty).collect(),
        }
    }

    fn finish(self) {
        self.builder.finalize();
    }

    fn emit_program(&mut self, program: &MirProgram) -> LangResult<()> {
        for statement in &program.body {
            self.emit_statement(statement)?;
            // top-level statements continue sequentially even if a helper marked the
            // block as terminated. Subsequent statements run in the same block.
            self.block_terminated = false;
        }
        Ok(())
    }

    fn emit_statement(&mut self, statement: &MirStatement) -> LangResult<()> {
        match statement {
            MirStatement::Let { var, init } => self.emit_let(*var, init.as_ref()),
            MirStatement::Assign { var, value } => self.emit_assign(*var, value),
            MirStatement::Echo(expr) => self.emit_echo(expr),
            MirStatement::If {
                cond,
                then_body,
                else_body,
            } => self.emit_if(cond, then_body, else_body),
            MirStatement::While { cond, body } => self.emit_while(cond, body),
            MirStatement::Break => self.emit_break(),
            MirStatement::Continue => self.emit_continue(),
        }
    }

    fn emit_let(&mut self, var: VarId, init: Option<&MirExpr>) -> LangResult<()> {
        let cranelift_var = Variable::from_u32(var.0 as u32);
        let ty = self.variable_types[var.0];
        let cl_ty = match ty {
            MirType::Int | MirType::Bool => types::I64,
        };
        self.builder.declare_var(cranelift_var, cl_ty);
        self.variable_defs[var.0] = Some(VariableInfo {
            variable: cranelift_var,
            ty,
        });
        let value = match init {
            Some(expr) => self.emit_expr(expr)?.value,
            None => self.default_value(ty),
        };
        self.builder.def_var(cranelift_var, value);
        Ok(())
    }

    fn emit_assign(&mut self, var: VarId, expr: &MirExpr) -> LangResult<()> {
        let info = *self.variable_info(var)?;
        let value = self.emit_expr(expr)?.value;
        self.builder.def_var(info.variable, value);
        Ok(())
    }

    fn emit_echo(&mut self, expr: &MirExpr) -> LangResult<()> {
        let value = self.emit_expr(expr)?;
        match value.ty {
            MirType::Int => self.print_int(value.value),
            MirType::Bool => self.print_bool(value.value),
        }
    }

    fn emit_if(
        &mut self,
        cond: &MirExpr,
        then_body: &[MirStatement],
        else_body: &[MirStatement],
    ) -> LangResult<()> {
        let condition_value = self.emit_expr(cond)?.value;
        let condition = self.bool_to_cond(condition_value);
        let current = self.builder.current_block().expect("must be in block");
        let then_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        let else_block = if else_body.is_empty() {
            merge_block
        } else {
            self.builder.create_block()
        };

        self.builder
            .ins()
            .brif(condition, then_block, &[], else_block, &[]);
        self.builder.seal_block(current);

        self.builder.switch_to_block(then_block);
        let then_terminated = self.emit_block(then_body)?;
        if !then_terminated {
            self.builder.ins().jump(merge_block, &[]);
        }
        self.builder.seal_block(then_block);

        if else_body.is_empty() {
            self.builder.switch_to_block(merge_block);
        } else {
            self.builder.switch_to_block(else_block);
            let else_terminated = self.emit_block(else_body)?;
            if !else_terminated {
                self.builder.ins().jump(merge_block, &[]);
            }
            self.builder.seal_block(else_block);
            self.builder.switch_to_block(merge_block);
        }
        self.builder.seal_block(merge_block);
        self.block_terminated = false;
        Ok(())
    }

    fn emit_while(&mut self, cond: &MirExpr, body: &[MirStatement]) -> LangResult<()> {
        let current = self.builder.current_block().expect("must be in block");
        let cond_block = self.builder.create_block();
        let body_block = self.builder.create_block();
        let exit_block = self.builder.create_block();

        self.builder.ins().jump(cond_block, &[]);
        self.builder.seal_block(current);

        self.builder.switch_to_block(cond_block);
        let condition_value = self.emit_expr(cond)?.value;
        let condition = self.bool_to_cond(condition_value);
        self.builder
            .ins()
            .brif(condition, body_block, &[], exit_block, &[]);
        self.builder.seal_block(cond_block);

        self.loop_stack.push(LoopContext {
            break_block: exit_block,
            continue_block: cond_block,
        });

        self.builder.switch_to_block(body_block);
        let body_terminated = self.emit_block(body)?;
        if !body_terminated {
            self.builder.ins().jump(cond_block, &[]);
        }
        self.builder.seal_block(body_block);
        self.loop_stack.pop();

        self.builder.switch_to_block(exit_block);
        self.builder.seal_block(exit_block);
        self.block_terminated = false;
        Ok(())
    }

    fn emit_break(&mut self) -> LangResult<()> {
        if let Some(context) = self.loop_stack.last() {
            self.builder.ins().jump(context.break_block, &[]);
            self.block_terminated = true;
            Ok(())
        } else {
            Err(LangError::Runtime("`break` outside of loop".into()))
        }
    }

    fn emit_continue(&mut self) -> LangResult<()> {
        if let Some(context) = self.loop_stack.last() {
            self.builder.ins().jump(context.continue_block, &[]);
            self.block_terminated = true;
            Ok(())
        } else {
            Err(LangError::Runtime("`continue` outside of loop".into()))
        }
    }

    fn emit_block(&mut self, statements: &[MirStatement]) -> LangResult<bool> {
        let previous = self.block_terminated;
        self.block_terminated = false;
        for statement in statements {
            if self.block_terminated {
                break;
            }
            self.emit_statement(statement)?;
        }
        let terminated = self.block_terminated;
        self.block_terminated = previous;
        Ok(terminated)
    }

    fn emit_expr(&mut self, expr: &MirExpr) -> LangResult<CodegenValue> {
        match &expr.kind {
            MirExprKind::Int(value) => {
                let val = self.builder.ins().iconst(types::I64, *value);
                Ok(CodegenValue::new(val, MirType::Int))
            }
            MirExprKind::Bool(value) => {
                let val = self
                    .builder
                    .ins()
                    .iconst(types::I64, if *value { 1 } else { 0 });
                Ok(CodegenValue::new(val, MirType::Bool))
            }
            MirExprKind::Var(var) => {
                let info = *self.variable_info(*var)?;
                let value = self.builder.use_var(info.variable);
                Ok(CodegenValue::new(value, info.ty))
            }
            MirExprKind::Unary { op, expr } => match op {
                MirUnaryOp::Negate => {
                    let value = self.emit_expr(expr)?;
                    let neg = self.builder.ins().ineg(value.value);
                    Ok(CodegenValue::new(neg, MirType::Int))
                }
                MirUnaryOp::Not => {
                    let value = self.emit_expr(expr)?;
                    let cmp = self.builder.ins().icmp_imm(IntCC::Equal, value.value, 0);
                    let bool_val = self.cond_to_i64(cmp);
                    Ok(CodegenValue::new(bool_val, MirType::Bool))
                }
            },
            MirExprKind::Binary { op, left, right } => self.emit_binary(*op, left, right),
        }
    }

    fn emit_binary(
        &mut self,
        op: MirBinaryOp,
        left: &MirExpr,
        right: &MirExpr,
    ) -> LangResult<CodegenValue> {
        match op {
            MirBinaryOp::Add => {
                self.emit_arith(left, right, |builder, l, r| builder.ins().iadd(l, r))
            }
            MirBinaryOp::Subtract => {
                self.emit_arith(left, right, |builder, l, r| builder.ins().isub(l, r))
            }
            MirBinaryOp::Multiply => {
                self.emit_arith(left, right, |builder, l, r| builder.ins().imul(l, r))
            }
            MirBinaryOp::Divide => {
                self.emit_arith(left, right, |builder, l, r| builder.ins().sdiv(l, r))
            }
            MirBinaryOp::Modulo => {
                self.emit_arith(left, right, |builder, l, r| builder.ins().srem(l, r))
            }
            MirBinaryOp::Equal => self.emit_compare(IntCC::Equal, left, right),
            MirBinaryOp::NotEqual => self.emit_compare(IntCC::NotEqual, left, right),
            MirBinaryOp::Less => self.emit_compare(IntCC::SignedLessThan, left, right),
            MirBinaryOp::LessEqual => self.emit_compare(IntCC::SignedLessThanOrEqual, left, right),
            MirBinaryOp::Greater => self.emit_compare(IntCC::SignedGreaterThan, left, right),
            MirBinaryOp::GreaterEqual => {
                self.emit_compare(IntCC::SignedGreaterThanOrEqual, left, right)
            }
            MirBinaryOp::And => self.emit_logical_and(left, right),
            MirBinaryOp::Or => self.emit_logical_or(left, right),
        }
    }

    fn emit_arith<F>(&mut self, left: &MirExpr, right: &MirExpr, op: F) -> LangResult<CodegenValue>
    where
        F: FnOnce(&mut cranelift_frontend::FunctionBuilder<'a>, Value, Value) -> Value,
    {
        let lhs = self.emit_expr(left)?;
        let rhs = self.emit_expr(right)?;
        let value = op(&mut self.builder, lhs.value, rhs.value);
        Ok(CodegenValue::new(value, MirType::Int))
    }

    fn emit_compare(
        &mut self,
        condition: IntCC,
        left: &MirExpr,
        right: &MirExpr,
    ) -> LangResult<CodegenValue> {
        let lhs = self.emit_expr(left)?;
        let rhs = self.emit_expr(right)?;
        let cmp = self.builder.ins().icmp(condition, lhs.value, rhs.value);
        let result = self.cond_to_i64(cmp);
        Ok(CodegenValue::new(result, MirType::Bool))
    }

    fn emit_logical_and(&mut self, left: &MirExpr, right: &MirExpr) -> LangResult<CodegenValue> {
        let lhs = self.emit_expr(left)?;
        let false_block = self.builder.create_block();
        let rhs_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I64);

        let current = self.builder.current_block().expect("must be in block");
        let lhs_cond = self.bool_to_cond(lhs.value);
        self.builder
            .ins()
            .brif(lhs_cond, rhs_block, &[], false_block, &[]);
        self.builder.seal_block(current);

        self.builder.switch_to_block(rhs_block);
        let rhs = self.emit_expr(right)?;
        self.builder.ins().jump(merge_block, &[rhs.value]);
        self.builder.seal_block(rhs_block);

        self.builder.switch_to_block(false_block);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(merge_block, &[zero]);
        self.builder.seal_block(false_block);

        self.builder.switch_to_block(merge_block);
        let result = self.builder.block_params(merge_block)[0];
        self.builder.seal_block(merge_block);
        Ok(CodegenValue::new(result, MirType::Bool))
    }

    fn emit_logical_or(&mut self, left: &MirExpr, right: &MirExpr) -> LangResult<CodegenValue> {
        let lhs = self.emit_expr(left)?;
        let true_block = self.builder.create_block();
        let rhs_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, types::I64);

        let current = self.builder.current_block().expect("must be in block");
        let lhs_cond = self.bool_to_cond(lhs.value);
        self.builder
            .ins()
            .brif(lhs_cond, true_block, &[], rhs_block, &[]);
        self.builder.seal_block(current);

        self.builder.switch_to_block(rhs_block);
        let rhs = self.emit_expr(right)?;
        self.builder.ins().jump(merge_block, &[rhs.value]);
        self.builder.seal_block(rhs_block);

        self.builder.switch_to_block(true_block);
        let one = self.builder.ins().iconst(types::I64, 1);
        self.builder.ins().jump(merge_block, &[one]);
        self.builder.seal_block(true_block);

        self.builder.switch_to_block(merge_block);
        let result = self.builder.block_params(merge_block)[0];
        self.builder.seal_block(merge_block);
        Ok(CodegenValue::new(result, MirType::Bool))
    }

    fn variable_info(&self, var: VarId) -> LangResult<&VariableInfo> {
        self.variable_defs[var.0]
            .as_ref()
            .ok_or_else(|| LangError::Runtime(format!("variable `{}` not defined", var.0)))
    }

    fn default_value(&mut self, ty: MirType) -> Value {
        match ty {
            MirType::Int => self.builder.ins().iconst(types::I64, 0),
            MirType::Bool => self.builder.ins().iconst(types::I8, 0),
        }
    }

    fn print_int(&mut self, value: Value) -> LangResult<()> {
        let gv = if let Some(gv) = self.int_format_global {
            gv
        } else {
            let new_gv = self
                .module
                .declare_data_in_func(self.declared.int_format, self.builder.func);
            self.int_format_global = Some(new_gv);
            new_gv
        };
        let format_ptr = self.builder.ins().global_value(self.pointer_type, gv);
        self.builder
            .ins()
            .call(self.printf_ref, &[format_ptr, value]);
        Ok(())
    }

    fn print_bool(&mut self, value: Value) -> LangResult<()> {
        let cond = self.bool_to_cond(value);
        let true_gv = if let Some(gv) = self.true_global {
            gv
        } else {
            let new_gv = self
                .module
                .declare_data_in_func(self.declared.true_str, self.builder.func);
            self.true_global = Some(new_gv);
            new_gv
        };
        let false_gv = if let Some(gv) = self.false_global {
            gv
        } else {
            let new_gv = self
                .module
                .declare_data_in_func(self.declared.false_str, self.builder.func);
            self.false_global = Some(new_gv);
            new_gv
        };
        let true_ptr = self.builder.ins().global_value(self.pointer_type, true_gv);
        let false_ptr = self.builder.ins().global_value(self.pointer_type, false_gv);
        let selected = self.builder.ins().select(cond, true_ptr, false_ptr);
        self.builder.ins().call(self.puts_ref, &[selected]);
        Ok(())
    }

    fn bool_to_cond(&mut self, value: Value) -> Value {
        self.builder.ins().icmp_imm(IntCC::NotEqual, value, 0)
    }

    fn cond_to_i64(&mut self, cond: Value) -> Value {
        let one = self.builder.ins().iconst(types::I64, 1);
        let zero = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().select(cond, one, zero)
    }
}

struct CodegenValue {
    value: Value,
    ty: MirType,
}

impl CodegenValue {
    fn new(value: Value, ty: MirType) -> Self {
        Self { value, ty }
    }
}

fn find_linker() -> Option<String> {
    for candidate in ["cc", "clang", "gcc"] {
        if which(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}
