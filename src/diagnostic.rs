//! Typst diagnostics resolved to source locations: a message, the place its
//! span points at, and the trace of calls and imports it passed through.

use std::fmt;

use typst::WorldExt as _;
use typst::diag::SourceDiagnostic;
use typst::syntax::{DiagSpan, FileId, VirtualRoot};

use crate::world::World;

/// A point in a source file: its path and 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Source path. Root-relative for a project file, or `{pkg-spec}/{vpath}`
    /// for a file imported from a package, so the identity is stable across
    /// consuming trees.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub column: usize,
}

/// One Typst diagnostic with its span and trace resolved to [`Location`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Typst's own message.
    pub message: String,
    /// Where the diagnostic's span points, when it resolves to a source file.
    pub location: Option<Location>,
    /// The trace from the span outward, innermost first: each call, show rule,
    /// import or include the error passed through on its way to the main file.
    pub trace: Vec<Location>,
}

impl Diagnostic {
    /// The first point of this diagnostic inside the file at `path`: its own
    /// location when that lies there, else the innermost trace point that
    /// does. An error raised inside an imported file thus lands on the line
    /// of `path` that led into it.
    #[must_use]
    pub fn first_in(&self, path: &str) -> Option<&Location> {
        self.location
            .iter()
            .chain(&self.trace)
            .find(|location| location.path == path)
    }
}

/// The diagnostics a failed evaluation produced, plus the path of the main
/// file it evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError {
    /// Path of the evaluated main file, in [`Location::path`] form.
    pub main: String,
    /// Every diagnostic, in the order Typst reported them.
    pub diagnostics: Vec<Diagnostic>,
}

impl EvalError {
    /// The first point inside the main file across the diagnostics, in order:
    /// where a reader of the main file should look.
    #[must_use]
    pub fn main_location(&self) -> Option<&Location> {
        self.diagnostics
            .iter()
            .find_map(|diagnostic| diagnostic.first_in(&self.main))
    }
}

impl fmt::Display for EvalError {
    /// The messages joined with `; `, or a generic message when there are none.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diagnostics.is_empty() {
            return f.write_str("unknown evaluation error");
        }
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            f.write_str(&diagnostic.message)?;
        }
        Ok(())
    }
}

impl World {
    /// Resolve Typst's diagnostics against this world into an [`EvalError`].
    #[must_use]
    pub fn eval_error(&self, diags: &[SourceDiagnostic]) -> EvalError {
        let diagnostics = diags
            .iter()
            .map(|diag| Diagnostic {
                message: diag.message.as_str().to_owned(),
                location: self.locate(diag.span),
                trace: diag
                    .trace
                    .iter()
                    .filter_map(|point| self.locate(point.span))
                    .collect(),
            })
            .collect();
        EvalError {
            main: file_path(typst::World::main(self)),
            diagnostics,
        }
    }

    /// Resolve a span to a [`Location`], or `None` when it is detached or
    /// points into a file that is not Typst source.
    #[must_use]
    pub fn locate(&self, span: impl Into<DiagSpan>) -> Option<Location> {
        let span = span.into();
        let file_id = span.id()?;
        let source = typst::World::source(self, file_id).ok()?;
        let range = self.range(span)?;
        let (line, column) = source.lines().byte_to_line_column(range.start)?;
        Some(Location {
            path: file_path(file_id),
            line: line + 1,
            column: column + 1,
        })
    }
}

/// A file id as a [`Location::path`]: root-relative for a project file,
/// package-qualified for a package file.
fn file_path(file_id: FileId) -> String {
    let vpath = file_id.vpath().get_with_slash();
    let vpath = vpath.strip_prefix('/').unwrap_or(vpath);
    match file_id.root() {
        VirtualRoot::Package(spec) => format!("{spec}/{vpath}"),
        VirtualRoot::Project => vpath.to_owned(),
    }
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;
