use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolRequest, CallToolResult, ListToolsRequest,
    ListToolsResult, RpcError,
};
use rust_mcp_sdk::{mcp_server::ServerHandler, McpServer};
use std::sync::Arc;

use crate::tools::ConverterTools;

#[derive(Default)]
pub struct ConverterServerHandler;

#[async_trait]
impl ServerHandler for ConverterServerHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: ConverterTools::tools(),
        })
    }

    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let tool_params: ConverterTools =
            ConverterTools::try_from(request.params).map_err(CallToolError::new)?;

        match tool_params {
            ConverterTools::ConvertDtsToUff(tool) => tool.call_tool().await,
        }
    }
}
