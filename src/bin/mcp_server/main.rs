mod handler;
mod tools;

use clap::Parser;
use handler::ConverterServerHandler;
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{server_runtime, ServerRuntime},
    McpServer, StdioTransport, TransportOptions,
};
use std::sync::Arc;

/// Start the DTS → UFF conversion MCP server over stdio.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Runs the DTS to UFF converter as a Model Context Protocol (MCP) server",
    long_about = "Launches a stdio-based MCP server that exposes the `convert_dts_to_uff` tool.
Use this binary when integrating the converter with clients that speak the MCP standard."
)]
struct Cli;

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Parse CLI arguments so `--help` and `--version` output is available to users.
    let _args = Cli::parse();

    let server_details = InitializeResult {
        server_info: Implementation {
            name: "dts-to-uff-converter".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("DTS to UFF Converter".to_string()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        meta: None,
        instructions: Some(
            "Use the `convert_dts_to_uff` tool to turn a DTS folder into a UFF Type 58 file. \
             Provide absolute paths that are readable by the server container. Track names can be\
             newline- or comma-separated in the supplied text file."
                .trim()
                .to_string(),
        ),
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = ConverterServerHandler;

    let server: Arc<ServerRuntime> =
        server_runtime::create_server(server_details, transport, handler);

    if let Err(start_error) = server.start().await {
        eprintln!(
            "{}",
            start_error
                .rpc_error_message()
                .unwrap_or(&start_error.to_string())
        );
    }

    Ok(())
}
