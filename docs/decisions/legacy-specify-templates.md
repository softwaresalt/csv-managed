---
title: Legacy Specify Templates Reference
status: Archived
date: 2026-05-21
superseded_by: .backlogit/templates/
---

These templates were part of the original `.specify/` system used before the
backlogit + autoharness migration. They are superseded by the templates in
`.backlogit/templates/`.

## Original Templates

| Template | Purpose | New Equivalent |
|----------|---------|----------------|
| `agent-file-template.md` | Agent definition scaffold | `.github/agents/*.agent.md` |
| `checklist-template.md` | Quality checklist scaffold | Review skill personas |
| `constitution-template.md` | Constitution scaffold | `.github/instructions/constitution.instructions.md` |
| `plan-template.md` | Implementation plan scaffold | `impl-plan` skill |
| `spec-template.md` | Feature specification scaffold | `docs/product-specs/` conventions |
| `tasks-template.md` | Task breakdown scaffold | `harvest` skill + backlogit templates |

These templates are no longer used. The backlogit system provides structured
templates at `.backlogit/templates/` for features, tasks, subtasks, shipments,
deliberations, and reviews.
