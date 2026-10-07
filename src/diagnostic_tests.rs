#![allow(clippy::unwrap_used)]

use comemo::Track as _;
use typst::World as _;
use typst::engine::{Route, Sink, Traced};

use typst::diag::{SourceDiagnostic, Tracepoint};
use typst::syntax::Span;

use super::{Diagnostic, EvalError, Hint, Location, Severity, TracePoint};
use crate::{World, WorldError};

/// Write `files` (path, body) into a fresh project rooted at a `Cargo.toml`.
fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    for (path, body) in files {
        let file = dir.path().join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, body).unwrap();
    }
    dir
}

/// Evaluate `world`'s main file, which must fail, and resolve its diagnostics.
fn eval(world: &World) -> EvalError {
    let world_dyn: &dyn typst::World = world;
    let source = world.source(world.main()).unwrap();
    let mut sink = Sink::new();
    let traced = Traced::default();
    let route = Route::default();
    let diags = typst_eval::eval(
        world_dyn.track(),
        world.library(),
        traced.track(),
        sink.track_mut(),
        route.track(),
        &source,
    )
    .err()
    .unwrap();
    world.eval_error(&diags)
}

/// Evaluate `main.typ` of a project holding `files`.
fn eval_error(files: &[(&str, &str)]) -> EvalError {
    let dir = project(files);
    eval(&World::new(&dir.path().join("main.typ")).unwrap())
}

fn at(path: &str, line: usize, column: usize) -> Location {
    Location {
        path: path.to_owned(),
        line,
        column,
    }
}

fn point(location: Location) -> TracePoint {
    TracePoint {
        message: "while calling `f`".to_owned(),
        location,
    }
}

fn error(message: &str, location: Location) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        message: message.to_owned(),
        location: Some(location),
        hints: Vec::new(),
        trace: Vec::new(),
    }
}

#[test]
fn a_syntax_error_names_its_line_and_column() {
    let err = eval_error(&[("main.typ", "= Title\n\n#let tags = (\"a\" \"b\")\n")]);
    assert_eq!(err.main, "main.typ");
    assert_eq!(err.to_string(), "expected comma");
    assert_eq!(err.main_location(), Some(&at("main.typ", 3, 17)));
}

#[test]
fn an_error_inside_an_import_lands_on_the_main_files_call() {
    let err = eval_error(&[
        (
            "lib/check.typ",
            "#let check(n) = {\n  assert(n > 1, message: \"too small\")\n}\n",
        ),
        (
            "main.typ",
            "#import \"lib/check.typ\": check\n\n#check(0)\n",
        ),
    ]);
    let first = err.diagnostics.first().unwrap();
    assert_eq!(first.location.as_ref().unwrap().path, "lib/check.typ");
    assert_eq!(first.location.as_ref().unwrap().line, 2);
    assert_eq!(err.main_location(), Some(&at("main.typ", 3, 2)));
}

#[test]
fn an_unexpected_argument_through_a_show_rule_names_the_argument() {
    let err = eval_error(&[
        ("lib/doc.typ", "#let doc(title: none, body) = body\n"),
        (
            "main.typ",
            "#import \"lib/doc.typ\": doc\n#show: doc.with(\n  titel: \"x\",\n)\n",
        ),
    ]);
    assert_eq!(err.to_string(), "unexpected argument: titel");
    assert_eq!(err.main_location(), Some(&at("main.typ", 3, 3)));
}

#[test]
fn a_package_file_is_named_by_its_spec() {
    let dir = project(&[
        (
            "pkg/typst.toml",
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n",
        ),
        ("pkg/lib.typ", "#let boom() = panic(\"no\")\n"),
        ("main.typ", "#import \"@local/demo:1.0.0\": boom\n#boom()\n"),
    ]);
    let world = World::new(&dir.path().join("main.typ"))
        .unwrap()
        .with_local_package("demo", dir.path().join("pkg"));
    let err = eval(&world);
    let first = err.diagnostics.first().unwrap();
    assert_eq!(
        first.location.as_ref().unwrap().path,
        "@local/demo:1.0.0/lib.typ"
    );
    assert_eq!(err.main_location(), Some(&at("main.typ", 2, 2)));
}

#[test]
fn first_in_walks_the_trace_innermost_first() {
    let diagnostic = Diagnostic {
        trace: vec![
            point(at("mid.typ", 4, 1)),
            point(at("main.typ", 2, 5)),
            point(at("main.typ", 7, 1)),
        ],
        ..error("boom", at("pkg/lib.typ", 9, 1))
    };
    assert_eq!(diagnostic.first_in("main.typ"), Some(&at("main.typ", 2, 5)));
    assert_eq!(
        diagnostic.first_in("pkg/lib.typ"),
        Some(&at("pkg/lib.typ", 9, 1))
    );
    assert_eq!(diagnostic.first_in("other.typ"), None);
}

#[test]
fn main_location_takes_the_first_diagnostic_that_reaches_main() {
    let elsewhere = error("first", at("lib.typ", 1, 1));
    let in_main = error("second", at("main.typ", 6, 2));
    let err = EvalError {
        main: "main.typ".to_owned(),
        diagnostics: vec![elsewhere, in_main],
    };
    assert_eq!(err.to_string(), "first; second");
    assert_eq!(err.main_location(), Some(&at("main.typ", 6, 2)));
}

#[test]
fn an_empty_error_says_so() {
    let err = EvalError {
        main: "main.typ".to_owned(),
        diagnostics: Vec::new(),
    };
    assert_eq!(err.to_string(), "unknown evaluation error");
    assert_eq!(err.main_location(), None);
}

#[test]
fn typst_hints_are_kept() {
    let err = eval_error(&[("main.typ", "#let a = 1\n#a-b\n")]);
    let first = err.diagnostics.first().unwrap();
    assert_eq!(first.severity, Severity::Error);
    assert_eq!(first.message, "unknown variable: a-b");
    assert!(
        first
            .hints
            .iter()
            .any(|hint| hint.message.contains("subtraction") && hint.location.is_none()),
        "{:?}",
        first.hints
    );
}

#[test]
fn a_trace_point_says_what_it_passed_through() {
    let err = eval_error(&[
        ("lib/check.typ", "#let check(n) = assert(n > 1)\n"),
        ("main.typ", "#import \"lib/check.typ\": check\n#check(0)\n"),
    ]);
    let first = err.diagnostics.first().unwrap();
    let call = first.trace.first().unwrap();
    assert_eq!(call.message, "while calling `check`");
    assert_eq!(call.location, at("main.typ", 2, 2));
}

#[test]
fn a_warning_keeps_its_severity_and_hints() {
    let dir = project(&[("main.typ", "#let x = 1\n")]);
    let world = World::new(&dir.path().join("main.typ")).unwrap();
    let warning = SourceDiagnostic::warning(Span::detached(), "careful")
        .with_hint("generic")
        .with_tracepoint(Tracepoint::Import("x".into()), Span::detached());
    let diagnostic = world.diagnostic(&warning);
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.severity.to_string(), "warning");
    assert_eq!(diagnostic.location, None);
    assert_eq!(
        diagnostic.hints,
        vec![Hint {
            message: "generic".to_owned(),
            location: None,
        }]
    );
    assert!(
        diagnostic.trace.is_empty(),
        "a detached point resolves nowhere"
    );
}

#[test]
fn a_location_displays_as_path_line_column() {
    assert_eq!(at("tasks/a.typ", 3, 17).to_string(), "tasks/a.typ:3:17");
}

#[test]
fn an_eval_world_error_displays_typsts_message_alone() {
    let err = WorldError::Eval(eval_error(&[("main.typ", "#let t = (\"a\" \"b\")\n")]));
    assert_eq!(err.to_string(), "expected comma");
}
