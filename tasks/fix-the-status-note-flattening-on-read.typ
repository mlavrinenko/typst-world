#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "fix the status note flattening on read",
  status: done(
    2026,
    9,
    14,
  )[content\_str routes through plain\_text\_separated; Par/ListItem/EnumItem/TermItem boundaries cost a separator. 44 tests green, just check clean, 0.3.2 in CHANGELOG.],
)

== Summary

`content_str` (`src/value.rs`) projected a `Content` value with
`content.plain_text()` alone. That built-in only concatenates the leaf text
runs it finds while walking the tree, inserting nothing between a paragraph
break or a list item and the text before it. A mindtape status note authored
as a trailing `[…]` block with a blank-line paragraph break and a bulleted
list — `done(2026, 9, 14)[First line.\n\nSecond paragraph.\n\n- a bullet\n-
another]` — came back through `mt ls --format json` as one run-on string:
"First line.Second paragraph.a bulletanother". No space, no separator, list
structure gone.

Found while closing mindtape's own
`truth/tasks/fix-the-status-note-flattening-on-read.typ`.

== Scope

`plain_text_separated` replaces the bare `plain_text()` call: it still walks
via `Content::traverse` and still defers to the `PlainText` trait for leaf
text, but a node that `is::<ParElem>()`, `is::<ListItem>()`,
`is::<EnumItem>()`, or `is::<TermItem>()` costs a separating space first
(only when the buffer is non-empty and does not already end in whitespace,
so it never introduces a leading space). `ecow` becomes a direct dependency
— already in the tree transitively via `typst`, so no new crate — because
`PlainText::plain_text` writes into an `&mut EcoString`, not a `String`.

