//! Shared formatting utilities for MCP tool outputs

use crate::types::Node;

/// Format a single node as a list item with location
pub fn format_node_list_item(node: &Node) -> String {
    format!(
        "- **{}** `{}` - {}:{}-{}",
        node.kind.as_str(),
        node.name,
        node.file_path,
        node.start_line,
        node.end_line
    )
}

/// Format a node with signature
pub fn format_node_with_signature(node: &Node) -> String {
    let mut output = format_node_list_item(node);
    if let Some(ref sig) = node.signature {
        output.push_str(&format!("\n  `{}`", sig));
    }
    output.push('\n');
    output
}

/// Format a node with basic location (for callers/callees)
pub fn format_node_simple(node: &Node) -> String {
    format!(
        "- **{}** `{}` - {}:{}",
        node.kind.as_str(),
        node.name,
        node.file_path,
        node.start_line
    )
}

/// Normalize file path (remove leading ./)
pub fn normalize_path(path: &str) -> &str {
    path.trim_start_matches("./")
}

