// ABOUTME: LayeredSelector — deterministic fast path, escalating to an LLM classifier only when rules can't confidently narrow.
// ABOUTME: This is the cost-cascade applied to tool selection: cheap-first, pay for the model only on ambiguous turns.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use crate::deterministic::DeterministicSelector;
use crate::selector::{Classification, ToolClassifier, ToolSelector};
use crate::types::{SelectionOutcome, SelectionReason, SelectionRequest};

/// Default ceiling: if deterministic rules keep more than this fraction of the
/// candidates, they failed to narrow meaningfully and the turn is escalated.
const DEFAULT_ESCALATE_KEPT_RATIO: f32 = 0.75;

/// Composes a [`DeterministicSelector`] with a [`ToolClassifier`].
///
/// The cheap deterministic pass runs first, and the (costly) classifier is
/// consulted only when the deterministic result is untrustworthy — it fell back
/// to the full set, or it narrowed so little that paying for the model is
/// worthwhile.
///
/// A classifier failure is never fatal: the selector degrades to the
/// deterministic outcome.
pub struct LayeredSelector {
    deterministic: DeterministicSelector,
    classifier: Arc<dyn ToolClassifier>,
    escalate_kept_ratio: f32,
}

impl LayeredSelector {
    /// Compose a deterministic selector with a classifier, using the default
    /// escalation ratio.
    #[must_use]
    pub fn new(deterministic: DeterministicSelector, classifier: Arc<dyn ToolClassifier>) -> Self {
        Self {
            deterministic,
            classifier,
            escalate_kept_ratio: DEFAULT_ESCALATE_KEPT_RATIO,
        }
    }

    /// Override the kept-ratio above which the deterministic pass is considered
    /// to have under-narrowed. The value is clamped to `0.0..=1.0`.
    #[must_use]
    pub fn with_escalate_kept_ratio(mut self, ratio: f32) -> Self {
        self.escalate_kept_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Decide whether the deterministic outcome warrants paying for the model.
    ///
    /// The deterministic pass is untrustworthy — and worth escalating — when:
    /// - it fell back to the full set (`Fallback`),
    /// - it under-narrowed, keeping more than `escalate_kept_ratio` of the
    ///   candidates (the rules failed to discriminate), or
    /// - only the pinned floor survived (no message keyword or persona scope
    ///   fired, so the rules contributed no relevance signal at all).
    fn should_escalate(
        &self,
        deterministic: &SelectionOutcome,
        request: &SelectionRequest,
    ) -> bool {
        if deterministic.reason == SelectionReason::Fallback {
            return true;
        }

        let pinned: HashSet<&str> = request.pinned.iter().map(String::as_str).collect();
        let only_floor_survived = !deterministic
            .keep
            .iter()
            .any(|name| !pinned.contains(name.as_str()));
        if only_floor_survived {
            return true;
        }

        let candidate_count = request.candidates.len();
        if candidate_count == 0 {
            return false;
        }
        // Cast is lossy only for absurd tool counts; ratios stay well-behaved.
        // `cast_precision_loss` is allowed workspace-wide for validated casts.
        let ratio = deterministic.keep.len() as f32 / candidate_count as f32;
        ratio > self.escalate_kept_ratio
    }

    /// Build an outcome from the classifier's verdict, intersecting its tools
    /// with the real candidates and unioning the pinned floor, then propagating
    /// the classifier's routing signals. Returns `None` when the result would
    /// fall below `min_keep`, signalling the caller to keep the deterministic
    /// outcome instead.
    fn outcome_from_classifier(
        request: &SelectionRequest,
        classification: &Classification,
    ) -> Option<SelectionOutcome> {
        let chosen: HashSet<&str> = classification.tools.iter().map(String::as_str).collect();
        let pinned: HashSet<&str> = request.pinned.iter().map(String::as_str).collect();

        let keep: Vec<String> = request
            .candidates
            .iter()
            .filter(|c| chosen.contains(c.name.as_str()) || pinned.contains(c.name.as_str()))
            .map(|c| c.name.clone())
            .collect();

        if keep.len() < request.min_keep {
            return None;
        }

        let mut outcome =
            SelectionOutcome::from_kept(&request.candidates, keep, SelectionReason::Llm);
        outcome.model_tier = classification.model_tier;
        if let Some(grounded) = classification.needs_grounded_data {
            outcome.needs_grounded_data = grounded;
        }
        Some(outcome)
    }
}

#[async_trait]
impl ToolSelector for LayeredSelector {
    async fn select(&self, request: &SelectionRequest) -> SelectionOutcome {
        let deterministic = self.deterministic.select(request).await;
        if !self.should_escalate(&deterministic, request) {
            return deterministic;
        }

        match self
            .classifier
            .classify(&request.message, &request.candidates)
            .await
        {
            Ok(classification) => {
                Self::outcome_from_classifier(request, &classification).unwrap_or(deterministic)
            }
            Err(err) => {
                warn!(error = %err, "tool classifier failed; degrading to deterministic selection");
                deterministic
            }
        }
    }

    fn name(&self) -> &'static str {
        "layered"
    }
}
