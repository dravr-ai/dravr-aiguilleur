<!-- ABOUTME: Entry point for coding agents working in dravr-aiguilleur. -->
<!-- ABOUTME: Defers to .claude/CLAUDE.md for the full workflow, validation, and architecture rules. -->

# Agent Instructions

The canonical, detailed instructions live in [`.claude/CLAUDE.md`](.claude/CLAUDE.md).
Read it before making changes. The essentials:

- **Setup:** `git submodule update --init --recursive` then
  `git config core.hooksPath .build/hooks`.
- **Before every push:** `scripts/ci/pre-push-validate.sh` (fmt → clippy → `.build`
  validation → tests, then writes the marker the pre-push hook checks).
- **Never** create Pull Requests, use `--no-verify`, or reference AI assistance in
  commit messages.
- **Lints** mirror dravr-platform's full set; toolchain is pinned to **1.96.0**.
- **Architecture:** the core lib is provider-free; `dravr-tronc` is only the MCP
  transport shell. Keep the selection logic in the core so it can be linked
  in-process. Every new abstraction deletes what it replaces, in the same commit.
- **After pushing:** monitor CI to green before considering the work done.

## Limitation register (org-wide)

- A genuine, documented limitation in code carries `LIMITATION(registre#<issue>):` on the marker line, naming the limited item, backed by an issue in the **private** `dravr-ai/dravr-registre` tracker (labels `limitation` + this repo's name). Most dravr repos are PUBLIC — internal gaps and security residuals never go on this repo's own tracker.
- Deferral/confession prose ("for now", "not yet implemented", "is the follow-up", "in a follow-up commit", "not yet wired", "not threaded through") is CI-gated by `.build/validation/limitation-gates.sh` (invoked by `validate.sh`); a registered marker line is the only exemption. Implement the real thing, or register the gap — never document it unregistered.
- A capability declared but consumed only by tests is a phantom surface: wire a production consumer in the same change, or register it with a marker naming the item.
- A feature shipped disarmed (flag off, shadow/observe mode, log-only phase) gets a `feature-phases.yaml` entry (name/surface/current/advance_when/review_by); dravr-build-config's reusable `feature-phase-review` workflow opens a registre issue when the review date passes.
