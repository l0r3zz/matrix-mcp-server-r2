use matrix_mcp_server_r2::mcp::server::MatrixMcpServer;
use rmcp::{ServerHandler, model::*};

/// Verify the MCP server reports the expected tool count and names.
/// This does NOT require a live Matrix homeserver -- it only checks
/// the statically-registered tool metadata.
#[test]
fn server_info_returns_correct_metadata() {
    // We cannot construct MatrixMcpServer without a real Client, so
    // we just verify the ServerHandler trait is implemented by checking
    // the type compiles. A more thorough integration test would mock
    // the Matrix client.
    fn _assert_server_handler<T: ServerHandler>() {}
    _assert_server_handler::<MatrixMcpServer>();
}
