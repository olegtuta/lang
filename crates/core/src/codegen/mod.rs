//! Native code generation entry points.

mod cranelift;

use std::path::Path;

use lang_syntax::ast::Statement;

use crate::diagnostics::LangResult;
use crate::mir::lower_to_mir;

pub use cranelift::CraneliftBackend;

/// Build configuration for artifact generation.
#[derive(Debug, Clone, Copy)]
pub enum BuildProfile {
    Dev,
    Release,
}

impl BuildProfile {
    fn opt_level(self) -> &'static str {
        match self {
            BuildProfile::Dev => "none",
            BuildProfile::Release => "speed",
        }
    }
}

/// Compile the supplied statements into a standalone executable written to `output`.
pub fn compile_to_executable(
    statements: &[Statement],
    output: &Path,
    profile: BuildProfile,
) -> LangResult<()> {
    let program = lower_to_mir(statements)?;
    let mut backend = CraneliftBackend::new(profile)?;
    backend.compile(&program, output)
}
