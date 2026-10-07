#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "carry hints, severity and trace labels in diagnostics",
  status: done(
    2026,
    10,
    7,
  )[Diagnostic carries severity, hints and labelled trace points; World::diagnostic resolves one; WorldError::Eval displays Typst's message alone. 0.5.0.],
)

== Summary

`World::eval_error` keeps each diagnostic's message, location and trace
locations, and drops the rest of what Typst gave: its severity, its hints
(`SourceDiagnostic::hints`, some pointing at a secondary span) and what each
trace point was (a call, show rule, import or include). A caller cannot show
Typst's `hint:` lines. `WorldError::Eval` also displays as
`eval error: {message}`, so every caller inherits a prefix it did not choose.

== Scope

`Diagnostic` carries a `Severity`, its `hints` (each a message and an optional
`Location`) and its trace as labelled points. `World::diagnostic` resolves one
`SourceDiagnostic`, so a caller can resolve warnings too. `Location` displays as
`path:line:column`. `WorldError::Eval` displays as Typst's message alone, and a
caller renders hints and locations from the fields. Ships as 0.5.0 (breaking).
