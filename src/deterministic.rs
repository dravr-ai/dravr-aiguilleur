// ABOUTME: Zero-cost deterministic selector — keeps tools by pinned floor, persona category scope, and keyword→category rules.
// ABOUTME: The keyword taxonomy is caller-supplied so the library stays domain-agnostic (no fitness terms baked in).
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;

use crate::selector::ToolSelector;
use crate::types::{SelectionOutcome, SelectionReason, SelectionRequest};

/// Caller-defined map from a category label to the keywords that, when present
/// in a message, mark that category as relevant.
///
/// Matching is case-insensitive substring containment against the message, so
/// the keyword `"run"` activates on `"running"` and `"trail run"`. Choose
/// keyword lists deliberately to balance recall against false activations.
#[derive(Debug, Clone, Default)]
pub struct CategoryKeywordRules {
    by_category: HashMap<String, Vec<String>>,
}

impl CategoryKeywordRules {
    /// Start with no rules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add the keywords that activate `category`. Repeated calls for the same
    /// category extend its keyword list.
    #[must_use]
    pub fn with_category(
        mut self,
        category: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let entry = self.by_category.entry(category.into()).or_default();
        entry.extend(keywords.into_iter().map(|k| {
            let mut k = k.into();
            k.make_ascii_lowercase();
            k
        }));
        self
    }

    /// Return the set of categories activated by `message`.
    fn activated_categories(&self, message: &str) -> HashSet<&str> {
        let haystack = message.to_ascii_lowercase();
        self.by_category
            .iter()
            .filter(|(_, keywords)| keywords.iter().any(|k| haystack.contains(k.as_str())))
            .map(|(category, _)| category.as_str())
            .collect()
    }
}

/// Selects tools using only deterministic signals.
///
/// Uses the pinned floor, the persona's category scope, and keyword→category
/// rules. Never makes a network call — it is the fast path of a
/// [`LayeredSelector`](crate::LayeredSelector) and a complete selector in its
/// own right.
#[derive(Debug, Clone)]
pub struct DeterministicSelector {
    rules: CategoryKeywordRules,
}

impl DeterministicSelector {
    /// Construct a selector over the given keyword rules.
    #[must_use]
    pub fn new(rules: CategoryKeywordRules) -> Self {
        Self { rules }
    }

    /// Compute the kept names without wrapping them in an outcome — shared with
    /// the layered selector so both apply identical floor/scope semantics.
    pub(crate) fn keep_names(&self, request: &SelectionRequest) -> HashSet<String> {
        let pinned: HashSet<&str> = request.pinned.iter().map(String::as_str).collect();
        let scoped: HashSet<&str> = request
            .scoped_categories
            .iter()
            .map(String::as_str)
            .collect();
        let activated = self.rules.activated_categories(&request.message);

        request
            .candidates
            .iter()
            .filter(|c| {
                pinned.contains(c.name.as_str())
                    || scoped.contains(c.category.as_str())
                    || activated.contains(c.category.as_str())
            })
            .map(|c| c.name.clone())
            .collect()
    }
}

#[async_trait]
impl ToolSelector for DeterministicSelector {
    async fn select(&self, request: &SelectionRequest) -> SelectionOutcome {
        let kept = self.keep_names(request);

        // Refuse to starve the turn: if rules narrowed below the floor, the
        // signal was too weak to trust — expose everything instead.
        if kept.len() < request.min_keep {
            return SelectionOutcome::keep_all(&request.candidates, SelectionReason::Fallback);
        }

        // Preserve candidate order for stable, reproducible output.
        let keep = request
            .candidates
            .iter()
            .filter(|c| kept.contains(&c.name))
            .map(|c| c.name.clone())
            .collect();

        SelectionOutcome::from_kept(&request.candidates, keep, SelectionReason::Deterministic)
    }

    fn name(&self) -> &'static str {
        "deterministic"
    }
}
