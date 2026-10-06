#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "resolve eval diagnostics to source locations",
  status: done(
    2026,
    10,
    7,
  )[WorldError::Eval carries an EvalError with each diagnostic's location and trace; main\_location walks the trace into the main file. 0.4.0.],
)

== Summary

`format_diagnostics` (`src/world.rs`) kept each diagnostic's message and
dropped its span and trace, so a caller could say that a file failed to
evaluate but not where. MindTape printed
`tasks/missing-comma.typ: eval error: expected comma` for a 30-line task, and
the reader had to find the missing comma by eye.

== Scope

`WorldError::Eval` carries an `EvalError`: every `Diagnostic` with its message,
the `Location` (path, line, column) of its span, and its trace, innermost first.
`EvalError::main_location` names the first point inside the main file, walking
the trace when the error was raised in an imported file or package, so a caller
never has to show a line of a file its user did not write.

Found while closing MindTape's
`truth/tasks/name-the-line-of-a-task-files-eval-error.typ`.
