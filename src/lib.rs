// ABOUTME: dravr-aiguilleur core — a pluggable SPI for narrowing an LLM turn's tool set (and, later, routing the model).
// ABOUTME: Domain-agnostic and provider-free: the MCP service shell and any in-process host wire it to their registry and model.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

//! # dravr-aiguilleur
//!
//! An *aiguilleur* (railway switchman) routes traffic onto the right track.
//! This crate routes an LLM turn onto the right **tools** — narrowing a large
//! candidate set down to what a given message actually needs, so the model
//! sees a focused, relevant tool list instead of everything at once.
//!
//! Shipping every tool definition on every turn bloats context, degrades tool
//! selection accuracy, and wastes tokens. Empirically, retrieval-style tool
//! selection stays reliable up to ~30 tools and degrades beyond that, so a
//! relevance pre-filter matters well before a registry reaches three digits.
//!
//! ## The SPI
//!
//! [`ToolSelector`] is the single seam every selection strategy implements:
//!
//! - [`DeterministicSelector`] — zero-cost rules (pinned floor, persona category
//!   scope, keyword→category matching). No network calls.
//! - [`LayeredSelector`] — runs the deterministic pass first and escalates to an
//!   LLM [`ToolClassifier`] only when the rules cannot confidently narrow.
//!
//! The core deliberately has **no LLM provider dependency**: an LLM-backed
//! selector plugs in through the abstract [`ToolClassifier`] seam, so hosts wire
//! whatever model they use without this crate growing a provider dependency.
//!
//! ## Contract
//!
//! Every [`ToolSelector`] guarantees its kept set is a subset of the candidates,
//! always includes the `pinned` floor, and never drops below `min_keep` — it
//! returns the full set rather than starve a turn.

mod deterministic;
mod layered;
mod selector;
mod types;

pub use deterministic::{CategoryKeywordRules, DeterministicSelector};
pub use layered::LayeredSelector;
pub use selector::{Classification, ClassifyError, ToolClassifier, ToolSelector};
pub use types::{
    ModelTier, SelectionOutcome, SelectionReason, SelectionRequest, ToolCandidate,
    ToolCapabilityHints,
};
