//! Typst diagnostics as plain data: severity, message, the place the span
//! points at, Typst's hints, and the trace of calls and imports the problem
//! passed through. Callers render them; `Display` gives Typst's message alone.

use std::fmt;

use typst::WorldExt as _;
use typst::diag::SourceDiagnostic;
use typst::syntax::{DiagSpan, FileId, VirtualRoot};

use crate::world::World;

/// A point in a source file: its path and 1-based line and column.
///
/// Displays as `path:line:column`.
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

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.path, self.line, self.column)
    }
}

/// How serious a [`Diagnostic`] is. Displays as `error` or `warning`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Severity {
    /// A fatal error.
    Error,
    /// A non-fatal warning.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// One of Typst's hints on a [`Diagnostic`]. Displays as its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// Typst's hint text, without a `hint:` prefix.
    pub message: String,
    /// The secondary piece of code the hint is about, or `None` for a
    /// generic hint (Typst's CLI lists those as `hint: …` lines).
    pub location: Option<Location>,
}

impl fmt::Display for Hint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// One step of a [`Diagnostic`]'s trace: a call, show rule, import or include
/// the problem passed through. Displays as its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePoint {
    /// Typst's description of the step, such as ``while calling `check` ``
    /// or ``while importing `lib.typ` ``.
    pub message: String,
    /// Where the step happened.
    pub location: Location,
}

impl fmt::Display for TracePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// One Typst diagnostic with its spans resolved to [`Location`]s.
///
/// Displays as Typst's message alone; a caller that wants the location,
/// severity or hints renders them from the fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Whether this is an error or a warning.
    pub severity: Severity,
    /// Typst's own message.
    pub message: String,
    /// Where the diagnostic's span points, when it resolves to a source file.
    pub location: Option<Location>,
    /// Typst's hints, in the order it gave them.
    pub hints: Vec<Hint>,
    /// The trace from the span outward, innermost first: each call, show rule,
    /// import or include the problem passed through on its way to the main
    /// file. Points whose span resolves to no source file are left out.
    pub trace: Vec<TracePoint>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
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
            .chain(self.trace.iter().map(|point| &point.location))
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
        EvalError {
            main: file_path(typst::World::main(self)),
            diagnostics: diags.iter().map(|diag| self.diagnostic(diag)).collect(),
        }
    }

    /// Resolve one Typst diagnostic, error or warning, against this world.
    #[must_use]
    pub fn diagnostic(&self, diag: &SourceDiagnostic) -> Diagnostic {
        Diagnostic {
            severity: match diag.severity {
                typst::diag::Severity::Error => Severity::Error,
                typst::diag::Severity::Warning => Severity::Warning,
            },
            message: diag.message.as_str().to_owned(),
            location: self.locate(diag.span),
            hints: diag
                .hints
                .iter()
                .map(|hint| Hint {
                    message: hint.v.as_str().to_owned(),
                    location: self.locate(hint.span),
                })
                .collect(),
            trace: diag
                .trace
                .iter()
                .filter_map(|point| {
                    Some(TracePoint {
                        message: point.v.to_string(),
                        location: self.locate(point.span)?,
                    })
                })
                .collect(),
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
