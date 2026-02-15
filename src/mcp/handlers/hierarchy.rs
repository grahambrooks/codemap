//! Handler for hierarchy tool

use crate::db::Database;
use crate::mcp::format;
use crate::mcp::types::SymbolRequest;

pub fn handle_hierarchy(db: &Database, req: &SymbolRequest) -> String {
    match db.get_hierarchy(&req.symbol) {
        Ok(nodes) => {
            if nodes.is_empty() {
                format!("No hierarchy found for symbol '{}'", req.symbol)
            } else {
                let mut output = format!("# Hierarchy for '{}'\n\n", req.symbol);
                output.push_str(&format!("Found {} related symbols:\n\n", nodes.len()));
                for node in nodes {
                    output.push_str(&format::format_node(&node));
                    output.push_str("\n\n");
                }
                output
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}
