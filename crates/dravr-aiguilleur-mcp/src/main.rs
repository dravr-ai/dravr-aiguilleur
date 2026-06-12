// ABOUTME: CLI entry point for the dravr-aiguilleur MCP server binary.
// ABOUTME: Parses arguments, selects transport (stdio or HTTP), and starts serving the select_tools tool.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 dravr.ai

//! Binary entry point for the dravr-aiguilleur MCP server.

use std::error::Error;
use std::sync::Arc;

use clap::Parser;
use dravr_aiguilleur_mcp::{build_tool_registry, ServerState};
use dravr_tronc::mcp::transport::{http, stdio};
use dravr_tronc::server::cli::McpArgs;
use dravr_tronc::server::tracing_init;
use dravr_tronc::McpServer;
use tokio::sync::RwLock;

/// dravr-aiguilleur-mcp — MCP server exposing tool selection over the Model Context Protocol.
#[derive(Parser)]
#[command(name = "dravr-aiguilleur-mcp", version, about)]
struct Cli {
    #[command(flatten)]
    server: McpArgs,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();
    tracing_init::init(&cli.server.transport);

    let state = Arc::new(RwLock::new(ServerState));
    let registry = build_tool_registry();
    let server = Arc::new(McpServer::new(
        "dravr-aiguilleur-mcp",
        env!("CARGO_PKG_VERSION"),
        registry,
        state,
    ));

    tracing::info!(transport = %cli.server.transport, "Starting dravr-aiguilleur MCP server");

    match cli.server.transport.as_str() {
        "stdio" => stdio::run(server).await?,
        "http" => http::serve(server, &cli.server.host, cli.server.port).await?,
        other => {
            return Err(format!("Unknown transport: {other}. Valid: stdio, http").into());
        }
    }

    Ok(())
}
