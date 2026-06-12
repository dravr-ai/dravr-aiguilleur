<!-- ABOUTME: Public README for dravr-aiguilleur — what it is, the SPI, the three selection approaches, and the MCP service. -->
<!-- ABOUTME: Documents the deterministic (shipped), layered/LLM, and semantic approaches plus the concrete classifier integration. -->

# dravr-aiguilleur

An **aiguilleur** (railway switchman) routes traffic onto the right track. This
crate routes an LLM turn onto the right **tools** — narrowing a large candidate
set down to what a message actually needs — and, in a later phase, onto the
right **model**.

## Why

Shipping every tool definition on every turn bloats context, degrades tool-
selection accuracy, and wastes tokens. The effect is measurable well before a
registry reaches three digits: retrieval-style tool selection stays reliable up
to ~30 tools and degrades beyond that ([RAG-MCP], arXiv 2505.03275), and
Anthropic's own guidance is to keep only your 3–5 most-used tools loaded and
defer the rest past ~10 ([Tool Search Tool]). A relevance pre-filter is the fix.

`dravr-aiguilleur` is the server-side version of that pre-filter, exposed as a
small, pluggable SPI so any host can choose how clever the selection should be.

## The SPI

[`ToolSelector`] is the single seam every strategy implements:

```rust
#[async_trait]
pub trait ToolSelector {
    async fn select(&self, request: &SelectionRequest) -> SelectionOutcome;
    fn name(&self) -> &'static str;
}
```

A selector receives the message, the candidate tools, a `pinned` floor (names
that must always survive), the persona's `scoped_categories`, and a `min_keep`
lower bound. It returns a `SelectionOutcome` — the *turn plan*: the kept tool
set, the dropped set (for audit), a `reason`, an optional `model_tier`, and a
`needs_grounded_data` flag.

### Contract

Every `ToolSelector` guarantees its kept set is a subset of the candidates,
always includes the `pinned` floor, and never drops below `min_keep` — it returns
the full set rather than starve a turn.

## The three approaches

| | Approach | Cost | Status |
|---|---|---|---|
| **A** | **Deterministic** — `pinned` floor ∪ persona category scope ∪ keyword→category rules | zero (no network) | **shipped** |
| **#2** | **Layered** — deterministic fast path, escalate to an LLM classifier only when rules can't confidently narrow | one cheap LLM call on ambiguous turns | **shipped** (needs a classifier) |
| **B** | **Semantic** — embedding/BM25 retrieval over tool descriptions ([RAG-MCP]) | embedding call + index | **documented, not built** |

### A — Deterministic (shipped)

```rust
use dravr_aiguilleur::{CategoryKeywordRules, DeterministicSelector, ToolSelector};

let rules = CategoryKeywordRules::new()
    .with_category("data", ["activity", "run", "ride"])
    .with_category("nutrition", ["eat", "food", "meal"]);

let outcome = DeterministicSelector::new(rules).select(&request).await;
```

A category is kept when a message keyword activates it, the persona is scoped to
it, or a candidate in it is pinned. If that narrows below `min_keep`, the selector
falls back to the full set rather than guess. Equivalent to Anthropic's BM25/regex
tier — fully deterministic, zero latency.

### #2 — Layered (shipped; the recommended default once a classifier exists)

```rust
let selector = LayeredSelector::new(DeterministicSelector::new(rules), classifier);
```

The deterministic pass runs first. The classifier is consulted **only** when the
result is untrustworthy — it fell back, kept more than 75% of candidates, or only
the pinned floor survived (no signal fired). A classifier failure is never fatal:
the selector degrades to the deterministic outcome. This is the cost cascade
applied to tool selection: cheap-first, pay for the model only on ambiguous turns.

### B — Semantic (documented, not built)

The [RAG-MCP] approach: embed the message, cosine-rank against precomputed tool-
description vectors, keep top-k ∪ pinned. Highest fidelity, but requires an
embedding model and a vector index. The `ToolCandidate.keywords` and
`input_schema` fields exist so a future semantic selector has more to match on
than name + description. Not implemented in this crate yet.

## The LLM classifier seam (approach #2 wiring)

The core library has **no LLM provider dependency**. To make the `LayeredSelector`
escalate, a host implements [`ToolClassifier`]:

```rust
#[async_trait]
pub trait ToolClassifier {
    async fn classify(
        &self,
        message: &str,
        candidates: &[ToolCandidate],
    ) -> Result<Classification, ClassifyError>;
}

pub struct Classification {
    pub tools: Vec<String>,                  // relevant tool names
    pub model_tier: Option<ModelTier>,       // phase-2 routing signal
    pub needs_grounded_data: Option<bool>,   // override the derived gate
}
```

A concrete implementation wraps whatever model the host already uses (e.g. a
cheap structured-output call to Cohere, Gemini Flash-Lite, or a CLI runner) and
asks it to return the relevant tool names — and, in phase 2, a `ModelTier`. The
returned `Classification` is a *struct*, not a bare name list, so the same call
can inform both tool narrowing and model routing without a breaking change once
this crate is published. Returning `Classification::from_tools(names)` is the
phase-1 form; populating `model_tier` is the phase-2 form.

Because `classify` returns a `Result`, a model outage is a soft failure: the
layered selector logs it and keeps the deterministic result.

## Routing signals (phase 2)

`SelectionOutcome` carries two host routing signals beyond the tool set:

- **`needs_grounded_data`** — derived deterministically: true when any kept tool
  reads real data (`ToolCapabilityHints::is_data_grounded`). A host uses it to keep
  data-dependent turns on a faithful (non-fabricating) model.
- **`model_tier`** — `Light` / `Standard` / `Heavy`, the coarse model-capability a
  turn warrants. Vendor-neutral: the host maps each tier onto its own stack. Always
  `None` in phase-1 selection; populated by a classifier in phase 2.

## MCP service

`dravr-aiguilleur-mcp` wraps the core in a [`dravr-tronc`] MCP server exposing a
single stateless `select_tools` tool — each call carries its own candidates and
keyword rules, so it is a pure function of its input, convenient for experimenting
with selection scenarios in isolation.

```bash
cargo run -p dravr-aiguilleur-mcp -- --transport stdio
```

## License

Licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).

[`ToolSelector`]: https://docs.rs/dravr-aiguilleur
[`ToolClassifier`]: https://docs.rs/dravr-aiguilleur
[`dravr-tronc`]: https://github.com/dravr-ai/dravr-tronc
[RAG-MCP]: https://arxiv.org/abs/2505.03275
[Tool Search Tool]: https://www.anthropic.com/engineering/advanced-tool-use
