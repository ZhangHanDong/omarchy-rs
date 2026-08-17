# Repository guidance

Read `specs/project.spec.md` and the governing task Contract before changing
code. Requirements live in `knowledge/requirements/`; architectural rulings
live in `knowledge/decisions/`; human-facing explanations live in `docs/`.

This project is an optional user-space overlay. Do not edit the sibling Omarchy
checkout, overwrite official package files, or broaden a task into system
management. Preserve upstream behavior with differential tests and justify
optimization work with reproducible measurements.

Use synthetic fixtures and isolated HOME/PATH/state directories. No test may
read real agent logs, credentials, or user prompts.
