# Requirements

EARS/29148-style requirement records. Use one artifact per stable requirement or grouping requirement.

Required shape:
- `title:` is the canonical human-readable title used by graph, work-unit, and spec draft generation.
- `## Problem` explains the user or system problem.
- `## Requirements` is the normative source, with one `[REQ-NNN] ... MUST/SHOULD/MAY ...` clause per line.
- `## Scenarios` supplies the work-unit and draft-spec BDD source.
- `## Dependencies` declares ordering edges to other requirement ids.
- `## Open Questions` blocks executable work-unit generation when it contains real questions.

Specs link back via `satisfies:`.
