#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "write an HVal back as a Typst value, as Typst syntax and through serde",
  status: proposed(2026, 10, 28),
)

== Summary

`convert` turns a Typst `Value` into an `HVal`, and nothing goes back. A host
that hands a harvested value to Typst again, such as a check that calls a
Typst function on it, writes its own renderer to Typst syntax, and each one
gets string escapes, whole floats, one-element arrays and dates subtly
wrong. MindTape has two such renderers. A host that prints harvested values
as JSON, or reads them back from a cache, writes its own converters to and
from `serde_json::Value`, though the module calls `HVal` serde-style.

== Scope

- `HVal::to_value` is the inverse of `convert`: a `Date` becomes a
  `datetime`, everything else its plain counterpart.
- `HVal::to_typst` writes the value as Typst syntax through Typst's own
  `repr`, so evaluating it gives back an equal `HVal`.
- A `serde` feature derives `Serialize` and `Deserialize` for `HVal`. A
  `Date` serializes as its `YYYY-MM-DD` string, and a deserialized string is
  a `Str`. A number is an `Int` when it fits an `i64`, else a `Float`.
- How a host writes dates into its own files stays the host's call.

== Acceptance

- Evaluating `to_typst()` of a value with every variant, escapes, a whole
  float, an empty and a one-element array, and an odd dict key gives back
  the same `HVal`.
- With `serde`, JSON round trips every variant but `Date`, which comes back
  a `Str`.
- README and CHANGELOG name them; gates green; released.
