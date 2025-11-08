use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolRequest, CallToolResult, ListToolsRequest,
    ListToolsResult, RpcError,
};
use rust_mcp_sdk::{mcp_server::ServerHandler, McpServer};
use std::sync::Arc;

use crate::tools::{ConverterTools, ListDtsTracks};

#[derive(Default)]
pub struct ConverterServerHandler;

#[async_trait]
impl ServerHandler for ConverterServerHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        let mut tools = ConverterTools::tools();

        if let Some(tool) = tools
            .iter_mut()
            .find(|tool| tool.name == ListDtsTracks::tool_name())
        {
            if let Some(output_schema) = ListDtsTracks::output_schema() {
                tool.output_schema = Some(output_schema);
            }
        }

        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools,
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
            ConverterTools::ListDtsTracks(tool) => tool.call_tool().await,
        }
    }
}
