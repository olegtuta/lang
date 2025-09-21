use futures::executor::block_on;
use lang_core::task::{LangTask, TaskScheduler};
use lang_core::{LangResult, TypeRegistry, Value};
use lang_runtime::{ParallelExecutor, Scope};
use lang_syntax::parse_variable_declaration;

fn main() -> LangResult<()> {
    let registry = TypeRegistry::new();
    let source = "[int] $demo = 10;";
    let declaration = parse_variable_declaration(source, &registry)?;

    let mut scope = Scope::new();
    match declaration.value.clone() {
        Some(value) => {
            scope.declare_with_value(&declaration.name, declaration.ty.clone(), value)?
        }
        None => scope.declare(&declaration.name, declaration.ty.clone())?,
    }

    println!("Declared {} `{}`", declaration.ty, declaration.name);

    if let Some(value) = scope
        .get(&declaration.name)
        .and_then(|binding| binding.value())
    {
        println!("Initial value: {}", value);
    }

    let executor = ParallelExecutor::new()?;
    let task = LangTask::new(async { Ok(Value::from(5)) });
    let handle = block_on(executor.schedule(task))?;
    let result = block_on(handle.join())?;
    println!("Async task produced: {}", result);

    Ok(())
}
