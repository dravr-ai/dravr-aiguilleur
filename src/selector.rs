// ABOUTME: The ToolSelector SPI plus the ToolClassifier seam an LLM-backed selector plugs into.
// ABOUTME: Implementors narrow a turn's tool set; the LLM seam stays abstract so the core lib has no provider dependency.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use async_trait::async_trait;

use crate::types::{ModelTier, SelectionOutcome, SelectionRequest, ToolCandidate};

/// The selection service-provider interface: given a turn, return the subset of
/// tools to expose to the model.
///
/// Implementations range from zero-cost deterministic rules to an LLM
/// classifier; callers pick one (or compose them, see
/// [`LayeredSelector`](crate::LayeredSelector)) behind this single trait so the
/// surrounding pipeline never changes.
///
/// # Contract
///
/// Every implementation MUST guarantee that the returned
/// [`SelectionOutcome::keep`]:
/// - is a subset of `request.candidates` (by name), and
/// - contains every name in `request.pinned`, and
/// - has at least `request.min_keep` entries (or equals the full candidate set
///   when narrowing that far is not safe).
#[async_trait]
pub trait ToolSelector: Send + Sync {
    /// Narrow the request's candidate tools to the relevant subset for the turn.
    async fn select(&self, request: &SelectionRequest) -> SelectionOutcome;

    /// Stable name for logging and metrics (e.g. `"deterministic"`, `"layered"`).
    fn name(&self) -> &'static str;
}

/// Error returned by a [`ToolClassifier`] when it cannot produce a verdict.
///
/// A classifier failure is never fatal: the layered selector treats it as
/// "could not narrow" and falls back to the deterministic result.
#[derive(Debug, Clone)]
pub struct ClassifyError {
    /// Human-readable cause, safe to log (no secrets or raw provider payloads).
    pub message: String,
}

impl ClassifyError {
    /// Construct a classify error from any displayable cause.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ClassifyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "tool classifier error: {}", self.message)
    }
}

impl Error for ClassifyError {}

/// A classifier's verdict for a turn: the relevant tools plus optional routing
/// signals.
///
/// Returning a struct (rather than a bare name list) gives the phase-2 model
/// router a home in the same seam — `model_tier` and `needs_grounded_data` let
/// one classifier call inform both tool narrowing and model selection, without
/// a breaking change once the crate is published.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Classification {
    /// Names of tools the model judges relevant to the turn.
    pub tools: Vec<String>,
    /// Optional model tier the turn warrants (phase-2 router signal).
    pub model_tier: Option<ModelTier>,
    /// Optional override of the data-grounded signal; when `None`, the selector
    /// derives it from the kept tools' capabilities.
    pub needs_grounded_data: Option<bool>,
}

impl Classification {
    /// Construct a verdict carrying only the relevant tool names.
    #[must_use]
    pub fn from_tools(tools: Vec<String>) -> Self {
        Self {
            tools,
            model_tier: None,
            needs_grounded_data: None,
        }
    }
}

/// Abstract seam for the LLM-backed escalation step.
///
/// The core library deliberately does not depend on any LLM provider. A host
/// that wants the [`LayeredSelector`](crate::LayeredSelector) to escalate
/// ambiguous turns supplies a `ToolClassifier` that wraps whatever model it
/// uses (Cohere, Gemini, a CLI runner, …). The classifier returns a
/// [`Classification`]; the layered selector intersects its tools with the
/// candidate set and re-applies the pinned floor.
#[async_trait]
pub trait ToolClassifier: Send + Sync {
    /// Classify `message` against the candidate tools.
    ///
    /// # Errors
    ///
    /// Returns [`ClassifyError`] when the underlying model is unavailable or
    /// returns an unparseable response; callers treat this as a soft failure.
    async fn classify(
        &self,
        message: &str,
        candidates: &[ToolCandidate],
    ) -> Result<Classification, ClassifyError>;
}
