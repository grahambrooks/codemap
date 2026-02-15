//! Symbol search handler

use crate::db::Database;
use crate::mcp::constants::DEFAULT_SEARCH_LIMIT;
use crate::mcp::format::format_node_with_signature;
use crate::mcp::types::SearchRequest;

pub fn handle_search(db: &Database, req: &SearchRequest) -> String {
    let results = match db.search_nodes(&req.query, None, DEFAULT_SEARCH_LIMIT) {
        Ok(r) => r,
        Err(e) => return format!("Error: {}", e),
    };

    if results.is_empty() {
        return format!("No symbols found matching '{}'", req.query);
    }

    let mut output = format!(
        "Found {} symbols matching '{}':\n\n",
        results.len(),
        req.query
    );

    for node in results {
        output.push_str(&format_node_with_signature(&node));
    }

    output
}
