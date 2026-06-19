// ABOUTME: MCP server library for dravr-aiguilleur — exposes a request-driven `select_tools` tool over dravr-tronc.
// ABOUTME: Stateless and self-contained: each call carries its own candidates and keyword rules, ideal for experimentation.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

//! MCP server exposing [`dravr_aiguilleur`] tool selection over the Model
//! Context Protocol.
//!
//! Registers a single stateless `select_tools` tool: each call carries its own
//! candidates and keyword rules, so the server is a pure function of its input —
//! convenient for experimenting with selection scenarios in isolation.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dravr_aiguilleur::{
    CategoryKeywordRules, DeterministicSelector, SelectionRequest, ToolCandidate, ToolSelector,
};
use dravr_tronc::mcp::schema::{Tool, ToolResponse};
use dravr_tronc::mcp::tool::{McpTool, ToolContext, ToolRegistry};
use serde::Deserialize;
use serde_json::{json, Value};

/// Server state for the aiguilleur MCP server.
///
/// Selection is a pure function of each request, so the state carries no
/// session data — it exists to satisfy `dravr-tronc`'s state-generic server.
#[derive(Debug, Default)]
pub struct ServerState;

/// Arguments accepted by the `select_tools` MCP tool. Carries the full scenario
/// so the server stays stateless: candidates, the pinned floor, persona scope,
/// the `min_keep` lower bound, and the keyword rules to match against.
#[derive(Debug, Deserialize)]
struct SelectArgs {
    message: String,
    candidates: Vec<ToolCandidate>,
    #[serde(default)]
    pinned: Vec<String>,
    #[serde(default)]
    scoped_categories: Vec<String>,
    #[serde(default = "default_min_keep")]
    min_keep: usize,
    /// Map of category → activating keywords. Absent means relevance is decided
    /// only by the pinned floor and persona scope.
    #[serde(default)]
    rules: HashMap<String, Vec<String>>,
}

const fn default_min_keep() -> usize {
    1
}

/// The `select_tools` MCP tool: narrows a candidate tool set to the subset
/// relevant to a message using deterministic keyword/scope/pin rules.
struct SelectTool;

#[async_trait]
impl McpTool<ServerState> for SelectTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "select_tools".to_owned(),
            description: "Narrow a candidate tool set to the tools relevant to a message, \
                          using a pinned floor, persona category scope, and keyword rules."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["message", "candidates"],
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The user message to select tools for"
                    },
                    "candidates": {
                        "type": "array",
                        "description": "All tools the caller is willing to expose",
                        "items": {
                            "type": "object",
                            "required": ["name", "description", "category"],
                            "properties": {
                                "name": { "type": "string" },
                                "description": { "type": "string" },
                                "category": { "type": "string" }
                            }
                        }
                    },
                    "pinned": {
                        "type": "array",
                        "description": "Tool names that must always be kept",
                        "items": { "type": "string" }
                    },
                    "scoped_categories": {
                        "type": "array",
                        "description": "Categories the active persona is scoped to",
                        "items": { "type": "string" }
                    },
                    "min_keep": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Lower bound on kept tools before falling back to all"
                    },
                    "rules": {
                        "type": "object",
                        "description": "Map of category to its activating keywords",
                        "additionalProperties": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                }
            }),
            annotations: None,
        }
    }

    async fn execute(
        &self,
        _state: &Arc<ServerState>,
        _ctx: &ToolContext,
        arguments: Value,
    ) -> ToolResponse {
        let args: SelectArgs = match serde_json::from_value(arguments) {
            Ok(args) => args,
            Err(e) => return ToolResponse::error(format!("Invalid arguments: {e}")),
        };

        let rules = args.rules.into_iter().fold(
            CategoryKeywordRules::new(),
            |rules, (category, keywords)| rules.with_category(category, keywords),
        );

        let request = SelectionRequest {
            message: args.message,
            candidates: args.candidates,
            pinned: args.pinned,
            scoped_categories: args.scoped_categories,
            min_keep: args.min_keep.max(default_min_keep()),
        };

        let outcome = DeterministicSelector::new(rules).select(&request).await;

        match serde_json::to_string(&outcome) {
            Ok(payload) => ToolResponse::text(payload),
            Err(e) => ToolResponse::error(format!("Failed to serialize outcome: {e}")),
        }
    }
}

/// Build the MCP tool registry for the aiguilleur server.
#[must_use]
pub fn build_tool_registry() -> ToolRegistry<ServerState> {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(SelectTool));
    registry
}
