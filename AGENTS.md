# map-kit

Independent public MIT code. No private repository dependency. Preserve source and
asset licenses. Use synthetic fixtures only.
Use `rtk` for shell commands when available. Never publish user datasets or secrets.

User-approved test scope (2026-09-20): run affected feature/unit tests and needed
syntax/build/load checks; preserve relevant safety regressions. Do not rerun
unaffected suites for commits or docs. Docs-only work needs diff/reference/pin
checks. Detailed application, platform and performance acceptance is left to
the user. When integrated with a game Client, agent UI checks stop at basic
standalone startup. Full integration, recursive clean-clone and export matrices
require an explicit user request. Record known failures and user checks
separately; do not report unperformed tests as passed. This supersedes earlier
automatic full-suite/final-acceptance requirements.
