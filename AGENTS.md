# map-kit

Independent public MIT code with no private repository dependency. Preserve source
and asset licenses, use synthetic fixtures only, and never publish user datasets
or secrets. Use `rtk` for shell commands when available.

Contract: `spec/CURRENT_V1.md`. `.memap` and all other own formats are v1. Do not
add old readers, automatic upgrades or compatibility fallbacks, or increment
versions, without explicit user instruction. Build fingerprints invalidate
disposable caches; keep epochs/revisions for runtime ordering.

Validation: check only changed areas and their direct/indirect dependents; full
checks of any kind need an explicit user request. Docs-only work needs
diff/reference/pin checks. Consumer UI checks stop at basic standalone startup.
Record known failures and user checks separately; never report unperformed tests
as passed.

Documentation: `docs/README.md` is the index. Keep current contracts, usage,
open issues and essential validation in the owning document, editing existing
sections. Approved decision replacements are recorded only in the root
`docs/history/DECISIONS.md`; measurements and past comparisons live in Git
history. Active task notes belong in the root ignored `docs/tasks/`.

Commit verified work locally on main in dependency order with consumer pins and
the root lock. Never force-push or rewrite history.
