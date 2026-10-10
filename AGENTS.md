# map-kit

Independent public MIT code. No private repository dependency. Preserve source and
asset licenses. Use synthetic fixtures only.
Use `rtk` for shell commands when available. Never publish user datasets or secrets.

Validation: run affected feature/unit tests and needed
syntax/build/load checks; preserve relevant safety regressions. Do not rerun
unaffected suites for commits or docs. Docs-only work needs diff/reference/pin
checks. Detailed application, platform and performance acceptance is left to
the user. When integrated with a game Client, agent UI checks stop at basic
standalone startup. Full integration, recursive clean-clone and export matrices
require an explicit user request. Record known failures and user checks
separately; do not report unperformed tests as passed.

Current contract: `spec/CURRENT_V1.md`. `.memap` and all other own formats are v1. Only current algorithms are supported. Do not add old readers, automatic upgrades, compatibility fallbacks or increment versions without explicit user instruction. Build fingerprints invalidate disposable generated caches. Keep epochs/revisions for runtime ordering.

Validation scope: check only changed areas and directly/indirectly related areas.
Full checks of any kind require an explicit user request. This scope applies to
documentation, preparation and implementation work alike.

Documentation: use the local document index/current owning documents. Keep only
current contracts, usage, open issues and essential validation there; replace
existing sections instead of appending dated updates. Necessary historical
decisions/comparisons belong in `docs/history/` with a current-document link.
History is optional reading. Active task notes belong in root ignored
`docs/tasks/`; integrate results and delete finished notes/raw logs, without
archiving them. Preserve user data/maps, generated artifacts, models and fixtures.
Commit each verified task locally on main in dependency order, including consumer
pins and root lock. Push after the approved scope and checks finish. Never
force-push or rewrite history.
