// ABOUTME: Integration tests for the deterministic and layered tool selectors.
// ABOUTME: Verifies the pinned floor, persona scope, keyword activation, min_keep fallback, grounded-data signal, and LLM escalation.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

//! Integration tests for the deterministic and layered tool selectors.

use std::sync::Arc;

use async_trait::async_trait;
use dravr_aiguilleur::{
    CategoryKeywordRules, Classification, ClassifyError, DeterministicSelector, LayeredSelector,
    SelectionReason, SelectionRequest, ToolCandidate, ToolCapabilityHints, ToolClassifier,
    ToolSelector,
};

fn candidates() -> Vec<ToolCandidate> {
    let reads = ToolCapabilityHints {
        reads_data: true,
        requires_provider: true,
        ..ToolCapabilityHints::default()
    };
    vec![
        ToolCandidate::new("get_activities", "Fetch the user's activities", "data")
            .with_capabilities(reads),
        ToolCandidate::new("analyze_activity", "Analyze one activity", "analytics"),
        ToolCandidate::new("search_food", "Search the food database", "nutrition"),
        ToolCandidate::new("suggest_yoga_sequence", "Suggest a yoga flow", "mobility"),
        ToolCandidate::new("remember_fact", "Persist a coach memory", "memory"),
    ]
}

fn rules() -> CategoryKeywordRules {
    CategoryKeywordRules::new()
        .with_category("data", ["activity", "run", "ride"])
        .with_category("analytics", ["analyze", "analyse", "compare"])
        .with_category("nutrition", ["eat", "food", "meal"])
        .with_category("mobility", ["stretch", "yoga", "mobility"])
}

fn request(message: &str) -> SelectionRequest {
    SelectionRequest {
        message: message.to_owned(),
        candidates: candidates(),
        pinned: vec!["remember_fact".to_owned()],
        scoped_categories: vec![],
        min_keep: 1,
    }
}

#[tokio::test]
async fn deterministic_keeps_activated_category_and_pinned_floor() {
    let selector = DeterministicSelector::new(rules());
    let outcome = selector
        .select(&request("analyze my last running activity"))
        .await;

    assert_eq!(outcome.reason, SelectionReason::Deterministic);
    // "running" activates data, "analyze" activates analytics, remember_fact is pinned.
    assert!(outcome.keep.contains(&"get_activities".to_owned()));
    assert!(outcome.keep.contains(&"analyze_activity".to_owned()));
    assert!(outcome.keep.contains(&"remember_fact".to_owned()));
    // Nutrition and mobility are irrelevant to this turn — and recorded as dropped.
    assert!(!outcome.keep.contains(&"search_food".to_owned()));
    assert!(outcome.dropped.contains(&"search_food".to_owned()));
    assert!(outcome
        .dropped
        .contains(&"suggest_yoga_sequence".to_owned()));
    // get_activities reads provider data → the turn is data-grounded.
    assert!(outcome.needs_grounded_data);
    // Phase-1 selection never assigns a model tier.
    assert!(outcome.model_tier.is_none());
}

#[tokio::test]
async fn deterministic_not_grounded_when_no_data_tool_kept() {
    let selector = DeterministicSelector::new(rules());
    // Only nutrition + pinned memory survive; neither reads provider data.
    let mut req = request("what food should I eat");
    req.pinned = vec![];
    let outcome = selector.select(&req).await;

    assert_eq!(outcome.reason, SelectionReason::Deterministic);
    assert!(outcome.keep.contains(&"search_food".to_owned()));
    assert!(!outcome.needs_grounded_data);
}

#[tokio::test]
async fn deterministic_respects_persona_category_scope() {
    let selector = DeterministicSelector::new(rules());
    let mut req = request("tell me something");
    req.scoped_categories = vec!["nutrition".to_owned()];
    let outcome = selector.select(&req).await;

    // No keyword hit, but the persona is scoped to nutrition → keep that tool
    // (plus the pinned floor).
    assert!(outcome.keep.contains(&"search_food".to_owned()));
    assert!(outcome.keep.contains(&"remember_fact".to_owned()));
}

#[tokio::test]
async fn deterministic_falls_back_when_below_min_keep() {
    let selector = DeterministicSelector::new(rules());
    let mut req = request("hello there");
    req.pinned = vec![]; // remove the floor so nothing is kept
    req.min_keep = 2;
    let outcome = selector.select(&req).await;

    assert_eq!(outcome.reason, SelectionReason::Fallback);
    assert_eq!(outcome.keep.len(), candidates().len());
    assert!(outcome.dropped.is_empty());
}

struct StubClassifier {
    verdict: Classification,
    fail: bool,
}

#[async_trait]
impl ToolClassifier for StubClassifier {
    async fn classify(
        &self,
        _message: &str,
        _candidates: &[ToolCandidate],
    ) -> Result<Classification, ClassifyError> {
        if self.fail {
            Err(ClassifyError::new("model unavailable"))
        } else {
            Ok(self.verdict.clone())
        }
    }
}

#[tokio::test]
async fn layered_escalates_to_classifier_on_weak_deterministic_signal() {
    // A message with no keyword hits → deterministic keeps only the pinned
    // floor (no message-relevance signal fired), so the layered selector
    // escalates to the classifier.
    let classifier = Arc::new(StubClassifier {
        verdict: Classification::from_tools(vec!["search_food".to_owned()]),
        fail: false,
    });
    let selector = LayeredSelector::new(DeterministicSelector::new(rules()), classifier);

    let outcome = selector.select(&request("what's a good dinner idea")).await;

    assert_eq!(outcome.reason, SelectionReason::Llm);
    assert!(outcome.keep.contains(&"search_food".to_owned()));
    // Pinned floor survives the LLM pass.
    assert!(outcome.keep.contains(&"remember_fact".to_owned()));
}

#[tokio::test]
async fn layered_propagates_classifier_routing_signals() {
    use dravr_aiguilleur::ModelTier;

    let classifier = Arc::new(StubClassifier {
        verdict: Classification {
            tools: vec!["search_food".to_owned()],
            model_tier: Some(ModelTier::Light),
            needs_grounded_data: Some(false),
        },
        fail: false,
    });
    let selector = LayeredSelector::new(DeterministicSelector::new(rules()), classifier);

    let outcome = selector.select(&request("any dinner ideas")).await;

    assert_eq!(outcome.reason, SelectionReason::Llm);
    assert_eq!(outcome.model_tier, Some(ModelTier::Light));
    assert!(!outcome.needs_grounded_data);
}

#[tokio::test]
async fn layered_degrades_to_deterministic_when_classifier_fails() {
    let classifier = Arc::new(StubClassifier {
        verdict: Classification::default(),
        fail: true,
    });
    let selector = LayeredSelector::new(DeterministicSelector::new(rules()), classifier);

    // No-keyword message forces escalation; the classifier fails, so we fall
    // back to the deterministic result.
    let outcome = selector.select(&request("hi")).await;

    assert_ne!(outcome.reason, SelectionReason::Llm);
    assert!(outcome.keep.contains(&"remember_fact".to_owned()));
}
