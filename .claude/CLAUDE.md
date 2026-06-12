## Git Workflow: NO Pull Requests

## After Pushing — MANDATORY CI MONITORING

The Agent MUST monitor CI on every push and not consider work complete until the relevant workflows reach a terminal success status. If CI fails, fix and re-push in the same session. The shared pre-push hook (`.build/hooks/pre-push`) prints this reminder on success.

**Tool priority** (to preserve GitHub PAT rate-limit quota):
1. **WebFetch** the branch's Actions page at `https://github.com/dravr-ai/dravr-aiguilleur/actions?query=branch%3A<branch>` — no PAT quota cost
2. `gh run list --branch <branch>` or single targeted `gh run view <id>` — costs one core slot each
3. GitHub MCP tools (`mcp__github__*`) for non-read operations

**Forbidden:** `gh run watch`, background polling loops, sub-60s polling cadence.

**CRITICAL: NEVER create Pull Requests. All merges happen locally via squash merge.**

- NEVER use `gh pr create` or any PR creation command
- NEVER suggest creating a PR
- Feature branches are merged via local squash merge

### Workflow for Features
1. Create feature branch: `git checkout -b feature/my-feature`
2. Make commits, push to remote: `git push -u origin feature/my-feature`
3. When ready, squash merge locally (from main worktree):
   ```bash
   git checkout main
   git fetch origin
   git merge --squash origin/feature/my-feature
   git commit
   git push
   ```

### Bug Fixes
- Bug fixes go directly to `main` branch (no feature branch needed)
- Commit and push directly: `git push origin main`

## Mandatory Session Startup Checklist

Before touching any code in a new session, run in this order:

```bash
# 1. Pull shared build config (provides .build/hooks, .build/validation, etc.)
git submodule update --init --recursive

# 2. Set canonical git hooks path — ALWAYS .build/hooks, NEVER .githooks
git config core.hooksPath .build/hooks

# 3. Scan recent history for context
git log --oneline -10

# 4. Check CI health on main
gh run list --branch main --limit 10 --json workflowName,conclusion

# 5. See uncommitted work
git status
```

The canonical hooks/validation live in the `.build/` git submodule from
https://github.com/dravr-ai/dravr-build-config — never use a local `.githooks/`.

## Mandatory Pre-Push Validation

**Before EVERY push, run:**

```bash
scripts/ci/pre-push-validate.sh
```

It runs, in order: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `.build/validation/validate.sh`, and `cargo test --workspace`, then writes the `.git/validation-passed` marker the pre-push hook checks.

**NEVER use `--no-verify` when committing or pushing.** Hooks enforce SPDX headers, conventional commit messages (max 2 lines), no AI-generated commit signatures, and no unauthorized root markdown files.

### Test Output Verification — MANDATORY

After running ANY test command, verify tests actually ran:
- `running 0 tests` = wrong target → STOP and investigate
- `0 passed; 0 failed` = no tests executed → STOP and investigate

Never claim "tests pass" if 0 tests ran.

## Lint Configuration

The `[workspace.lints]` in `Cargo.toml` mirrors **dravr-platform's full set** (the
fleet standard): `unwrap_used` / `expect_used` / `panic` / `str_to_string` /
`cognitive_complexity` / `absolute_paths` / `disallowed_methods` denied;
`missing_docs` warned. The toolchain is pinned to **1.96.0** in
`rust-toolchain.toml`. Do not downgrade these to a lighter set.

## Project Overview

**dravr-aiguilleur** routes an LLM turn onto the right tools (and, in phase 2,
the right model). An *aiguilleur* is a railway switchman — the crate narrows a
large candidate tool set down to what a message actually needs.

### Workspace Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `dravr-aiguilleur` | library | Core SPI: `ToolSelector` trait, `DeterministicSelector`, `LayeredSelector`, `ToolClassifier` seam, DTOs. **Provider-free and tronc-free** — links in-process anywhere. |
| `dravr-aiguilleur-mcp` | library + binary | MCP server (stdio/HTTP) over `dravr-tronc` exposing a stateless `select_tools` tool for standalone experimentation. |

### Key Design Decisions
- **Core is provider-free**: no LLM provider dependency. An LLM-backed selector
  plugs in through the abstract `ToolClassifier` seam — the host supplies the model.
- **tronc is a transport, not the engine**: the selection logic lives in the core
  lib; the `dravr-tronc` `McpServer` is a thin shell over it, so the same core can
  be linked in-process to skip a network hop.
- **Selection contract**: a `ToolSelector`'s kept set is always ⊆ candidates,
  ⊇ the `pinned` floor, and never < `min_keep` (else it returns the full set).
- **Phase 1 is tool-only**: `model_tier` is carried but never assigned by phase-1
  selection — it is the home for the phase-2 model router.

# Writing code

- CRITICAL: NEVER USE `--no-verify` WHEN COMMITTING CODE
- We prefer simple, clean, maintainable solutions over clever or complex ones
- Make the smallest reasonable changes to get to the desired outcome
- When modifying code, match the style and formatting of surrounding code
- NEVER remove code comments unless you can prove that they are actively false
- All code files should start with a brief 2 line comment explaining what the file does. Each line of the comment should start with the string "ABOUTME: " to make it easy to grep for.
- When writing comments, avoid referring to temporal context about refactors or recent changes
- When fixing a bug or compilation error, NEVER throw away the old implementation and rewrite without explicit permission
- NEVER name things as 'improved' or 'new' or 'enhanced', etc. Code naming should be evergreen.
- NEVER add placeholder or dead_code or mock or name a variable starting with `_` (unused trait params excepted)
- NEVER use `#[allow(clippy::...)]` attributes EXCEPT for validated type-conversion casts
- NEVER introduce a new abstraction without deleting what it replaces in the same commit
- Be RUST idiomatic; do not hard code magic values
- Do not leave implementation with "In future versions" or "Fall back". Always implement the real thing.
- Commit without AI assistant-related commit messages. Do not reference AI assistance in git commits.
- Always create a branch when adding new features. Bug fixes go directly to main branch.
- Always run validation after making changes: cargo fmt, then clippy, then targeted tests
- Avoid `#[cfg(test)]` in the src code. Only in tests

## Error Handling Requirements

### Acceptable
- `?` operator for error propagation
- `Result<T, E>` for all fallible operations
- `Option<T>` for values that may not exist
- Custom error types implementing `std::error::Error`

### Prohibited
- `unwrap()` except in test code or static data known at compile time
- `expect()` — only for documenting invariants that should never fail
- `panic!()` — only in test assertions
- **`anyhow!()` macro** — ABSOLUTELY FORBIDDEN in all production code

## Mock Policy

- PREFER real implementations over mocks in all production code
- NEVER implement mock modes for production features
- Mocks (and test stubs) permitted ONLY in test code

## Documentation Standards

- All public APIs MUST have doc comments (`missing_docs = "warn"` is enforced)
- Document error conditions for `Result`-returning APIs (`# Errors`)
- Include usage examples for non-obvious APIs
