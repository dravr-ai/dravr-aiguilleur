// ABOUTME: Core data types for tool selection — the request a caller submits and the outcome (a turn plan) returned.
// ABOUTME: Domain-agnostic: candidates mirror a generic MCP tool schema; no fitness/Pierre concepts leak into the surface.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Routing-relevant capability hints a caller attaches to a candidate.
///
/// These mirror the small slice of an MCP tool's capabilities that influence
/// *routing* decisions — chiefly whether a tool pulls real user data, which
/// determines if a turn must go to a faithful (non-fabricating) model. Hosts
/// map their own richer capability model onto these booleans. Admin-only tools
/// are intentionally absent: those are filtered out before a turn ever reaches
/// selection, so a candidate is never admin-only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCapabilityHints {
    /// Tool reads real user/account data (activities, stats, sleep, …).
    #[serde(default)]
    pub reads_data: bool,
    /// Tool writes or mutates user data.
    #[serde(default)]
    pub writes_data: bool,
    /// Tool requires a connected third-party data provider.
    #[serde(default)]
    pub requires_provider: bool,
}

impl ToolCapabilityHints {
    /// Whether using this tool grounds the turn in real data — the signal a host
    /// uses to keep data-dependent turns on a faithful model rather than a
    /// cheaper one prone to fabricating metrics.
    #[must_use]
    pub const fn is_data_grounded(self) -> bool {
        self.reads_data || self.requires_provider
    }
}

/// A single tool the caller is willing to expose, described well enough for a
/// selector to judge its relevance to a turn.
///
/// `category` is a free-form bucket label (e.g. `"data"`, `"nutrition"`). The
/// library never interprets specific category values — callers define their own
/// taxonomy and the matching rules that reference it. The optional `keywords`
/// and `input_schema` give richer selectors (BM25, embedding, LLM) more to match
/// against without burdening the deterministic path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCandidate {
    /// Stable tool identifier the caller uses to dispatch (e.g. `get_activities`).
    pub name: String,
    /// One-line, model-facing description of what the tool does.
    pub description: String,
    /// Free-form grouping label used by deterministic matching rules.
    pub category: String,
    /// Extra match terms beyond the description (synonyms, domain jargon).
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Raw JSON input schema, for selectors that inspect parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    /// Routing-relevant capability hints.
    #[serde(default)]
    pub capabilities: ToolCapabilityHints,
}

impl ToolCandidate {
    /// Construct a minimal candidate from name, description, and category.
    /// Use the `with_*` builders to attach keywords, a schema, or capabilities.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: category.into(),
            keywords: Vec::new(),
            input_schema: None,
            capabilities: ToolCapabilityHints::default(),
        }
    }

    /// Attach match keywords.
    #[must_use]
    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Attach a raw JSON input schema.
    #[must_use]
    pub fn with_input_schema(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Attach routing capability hints.
    #[must_use]
    pub const fn with_capabilities(mut self, capabilities: ToolCapabilityHints) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// Coarse model-capability tier a router maps to a concrete provider+model.
///
/// The library is vendor-neutral: it expresses *how much model* a turn warrants,
/// and the host maps each tier onto its own stack (e.g. a light/economy model
/// for chit-chat, a frontier model for complex, data-grounded coaching). Phase 1
/// selection never emits a tier — it is the home for the phase-2 model router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Cheapest tier — short, low-stakes, non-data-grounded turns.
    Light,
    /// Mid tier — most everyday turns.
    Standard,
    /// Frontier tier — complex reasoning or faithful, data-grounded coaching.
    Heavy,
}

/// Everything a [`crate::ToolSelector`] needs to narrow a turn's tool set.
///
/// The selector must return a subset of `candidates` that always includes every
/// name in `pinned` (the non-negotiable floor), and never returns fewer than
/// `min_keep` tools — falling back to the full set when it cannot confidently
/// narrow that far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRequest {
    /// The latest user message, already locale-normalized by the caller.
    pub message: String,
    /// The full set of tools the caller is willing to expose this turn.
    pub candidates: Vec<ToolCandidate>,
    /// Tool names that must survive selection regardless of relevance
    /// (caller's core set ∪ tools the active persona explicitly references).
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Categories the active persona is scoped to, if any. Tools in these
    /// categories are always kept. Empty means "no persona scoping".
    #[serde(default)]
    pub scoped_categories: Vec<String>,
    /// Lower bound on the kept set. If a selector would keep fewer than this,
    /// it must fall back to the full candidate set instead of starving the turn.
    #[serde(default)]
    pub min_keep: usize,
}

/// Why a selector kept the set it returned — recorded for observability so a
/// caller can prove savings and audit fallbacks without re-running selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    /// Resolved by deterministic rules (keyword / category / pinned) alone.
    Deterministic,
    /// Resolved by an LLM classifier escalation.
    Llm,
    /// Could not narrow safely; returned the full candidate set.
    Fallback,
    /// Selection disabled by configuration; returned the full candidate set.
    Disabled,
}

/// The result of a selection pass — the turn plan a host acts on.
///
/// Carries both consumers of the one-classifier design: the kept tool set (for
/// narrowing the tool list) and the optional `model_tier` (for routing the
/// model). `dropped` and `needs_grounded_data` exist for host observability and
/// routing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionOutcome {
    /// Tool names to expose to the model this turn — a subset of the request's
    /// candidates, always a superset of its `pinned` floor.
    pub keep: Vec<String>,
    /// Tool names removed from the candidate set, for audit and savings metrics.
    pub dropped: Vec<String>,
    /// Why this set was chosen.
    pub reason: SelectionReason,
    /// Optional `0.0..=1.0` confidence, when the selector produces one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Optional model tier for the host's router. Always `None` in phase-1
    /// tool-only selection; populated by an LLM classifier in phase 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_tier: Option<ModelTier>,
    /// Whether any kept tool grounds the turn in real data — a host routing
    /// signal to keep such turns on a faithful model.
    pub needs_grounded_data: bool,
}

impl SelectionOutcome {
    /// Build an outcome from the kept names (in candidate order), deriving the
    /// `dropped` set and `needs_grounded_data` from the candidates. Leaves
    /// `confidence` and `model_tier` unset.
    #[must_use]
    pub fn from_kept(
        candidates: &[ToolCandidate],
        keep: Vec<String>,
        reason: SelectionReason,
    ) -> Self {
        let kept: HashSet<&str> = keep.iter().map(String::as_str).collect();
        let dropped = candidates
            .iter()
            .filter(|c| !kept.contains(c.name.as_str()))
            .map(|c| c.name.clone())
            .collect();
        let needs_grounded_data = candidates
            .iter()
            .filter(|c| kept.contains(c.name.as_str()))
            .any(|c| c.capabilities.is_data_grounded());

        Self {
            keep,
            dropped,
            reason,
            confidence: None,
            model_tier: None,
            needs_grounded_data,
        }
    }

    /// Build an outcome that keeps every candidate — the safe fallback used when
    /// a selector cannot narrow without risking starving the turn.
    #[must_use]
    pub fn keep_all(candidates: &[ToolCandidate], reason: SelectionReason) -> Self {
        let keep = candidates.iter().map(|c| c.name.clone()).collect();
        Self::from_kept(candidates, keep, reason)
    }
}
