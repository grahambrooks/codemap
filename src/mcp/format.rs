//! Shared formatting utilities for MCP tool outputs

use crate::types::{CodeNode, NodeKind};

/// Format a single node as a list item with location
pub fn format_node_list_item(node: &CodeNode) -> String {
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
pub fn format_node_with_signature(node: &CodeNode) -> String {
    let mut output = format_node_list_item(node);
    if let Some(ref sig) = node.signature {
        output.push_str(&format!("\n  `{}`", sig));
    }
    output.push('\n');
    output
}

/// Format a node with basic location (for callers/callees)
pub fn format_node_simple(node: &CodeNode) -> String {
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

/// Format node kind as a title
pub fn format_kind_title(kind: &NodeKind) -> String {
    kind.as_str().to_string()
}
