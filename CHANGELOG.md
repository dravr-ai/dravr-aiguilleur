<!-- ABOUTME: Changelog for dravr-aiguilleur, following Keep a Changelog. -->
<!-- ABOUTME: Documents notable changes per release of the tool-selection SPI and MCP service. -->

# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-19

### Changed
- `dravr-aiguilleur-mcp` migrated to `dravr-tronc` 0.5.3 (dual-era MCP engine):
  `mcp::schema::{Tool, ToolResponse}`, `McpTool::execute` gains a `&ToolContext`
  parameter, and tool state is `&Arc<S>` directly (tronc no longer wraps it in a
  `RwLock`). Consumed via the crates.io release instead of a git tag. The
  provider-free core `dravr-aiguilleur` crate is unchanged.

### Added
- Core `ToolSelector` SPI with the selection contract (kept set ⊆ candidates,
  ⊇ `pinned` floor, ≥ `min_keep`).
- `DeterministicSelector` — pinned floor, persona category scope, and
  keyword→category rules (`CategoryKeywordRules`).
- `LayeredSelector` — deterministic fast path escalating to a `ToolClassifier`
  only when the deterministic result is untrustworthy; degrades to deterministic
  on classifier failure.
- `ToolClassifier` seam returning a `Classification { tools, model_tier,
  needs_grounded_data }`, keeping the core free of any LLM provider dependency.
- Enriched `ToolCandidate` (keywords, raw `input_schema`, `ToolCapabilityHints`)
  and a `SelectionOutcome` turn plan (`keep`, `dropped`, `reason`, `confidence`,
  `model_tier`, `needs_grounded_data`).
- `dravr-aiguilleur-mcp` — a `dravr-tronc` MCP server exposing a stateless
  `select_tools` tool over stdio/HTTP.
