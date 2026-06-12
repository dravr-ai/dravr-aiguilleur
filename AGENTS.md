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
