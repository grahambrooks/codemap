//! Symbol information handlers (node, definition, references)

use std::fs;
use std::path::Path;

use crate::db::Database;
use crate::mcp::constants::{DEFAULT_CONTEXT_LINES, MAX_REFERENCES_PER_KIND};
use crate::mcp::types::{DefinitionRequest, SymbolRequest};
use crate::types::EdgeKind;

pub fn handle_node(db: &Database, req: &SymbolRequest) -> String {
    let node = match db.find_node_by_name(&req.symbol) {
        Ok(Some(n)) => n,
        Ok(None) => return format!("Symbol '{}' not found", req.symbol),
        Err(e) => return format!("Error: {}", e),
    };

    let mut output = format!("## {}: `{}`\n\n", node.kind.as_str(), node.name);

    output.push_str(&format!(
        "**File:** {}:{}-{}\n",
        node.file_path, node.start_line, node.end_line
    ));
    output.push_str(&format!("**Language:** {}\n", node.language.as_str()));
    output.push_str(&format!("**Visibility:** {}\n", node.visibility.as_str()));

    if node.is_async {
        output.push_str("**Async:** yes\n");
    }
    if node.is_static {
        output.push_str("**Static:** yes\n");
    }
    if node.is_exported {
        output.push_str("**Exported:** yes\n");
    }

    if let Some(ref sig) = node.signature {
        output.push_str(&format!("\n**Signature:**\n```\n{}\n```\n", sig));
    }

    if let Some(ref doc) = node.docstring {
        output.push_str(&format!("\n**Documentation:**\n{}\n", doc));
    }

    output
}

pub fn handle_definition(db: &Database, project_root: &str, req: &DefinitionRequest) -> String {
    let node = match db.find_node_by_name(&req.symbol) {
        Ok(Some(n)) => n,
        Ok(None) => return format!("Symbol '{}' not found", req.symbol),
        Err(e) => return format!("Error: {}", e),
    };

    let context_lines = req.context_lines.unwrap_or(DEFAULT_CONTEXT_LINES) as usize;

    // Read the source file
    let file_path = Path::new(project_root).join(&node.file_path);
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => return format!("Error reading file {}: {}", node.file_path, e),
    };

    let lines: Vec<&str> = content.lines().collect();
    let start = (node.start_line as usize).saturating_sub(1);
    let end = (node.end_line as usize).min(lines.len());

    if start >= lines.len() {
        return format!(
            "Error: line range {}-{} out of bounds",
            node.start_line, node.end_line
        );
    }

    // Build output with context
    let mut output = format!(
        "## {} `{}`\n\n**File:** {}:{}-{}\n**Language:** {}\n\n",
        node.kind.as_str(),
        node.name,
        node.file_path,
        node.start_line,
        node.end_line,
        node.language.as_str()
    );

    if let Some(ref sig) = node.signature {
        output.push_str(&format!("**Signature:** `{}`\n\n", sig));
    }

    // Context before
    let ctx_start = start.saturating_sub(context_lines);
    if ctx_start < start {
        output.push_str("```");
        output.push_str(node.language.as_str());
        output.push_str("\n// ... context before\n");
        for (i, line) in lines[ctx_start..start].iter().enumerate() {
            output.push_str(&format!("{:4} │ {}\n", ctx_start + i + 1, line));
        }
        output.push_str("// --- definition starts ---\n");
    } else {
        output.push_str("```");
        output.push_str(node.language.as_str());
        output.push('\n');
    }

    // The definition itself
    for (i, line) in lines[start..end].iter().enumerate() {
        output.push_str(&format!("{:4} │ {}\n", start + i + 1, line));
    }

    // Context after
    let ctx_end = (end + context_lines).min(lines.len());
    if ctx_end > end {
        output.push_str("// --- definition ends ---\n");
        for (i, line) in lines[end..ctx_end].iter().enumerate() {
            output.push_str(&format!("{:4} │ {}\n", end + i + 1, line));
        }
        output.push_str("// ... context after\n");
    }

    output.push_str("```\n");

    output
}

pub fn handle_references(db: &Database, req: &SymbolRequest) -> String {
    let node = match db.find_node_by_name(&req.symbol) {
        Ok(Some(n)) => n,
        Ok(None) => return format!("Symbol '{}' not found", req.symbol),
        Err(e) => return format!("Error: {}", e),
    };

    // Get all incoming edges (references TO this symbol)
    let edges = match db.get_incoming_edges(node.id) {
        Ok(e) => e,
        Err(e) => return format!("Error: {}", e),
    };

    if edges.is_empty() {
        return format!("No references found for '{}'", req.symbol);
    }

    let mut output = format!(
        "## References to `{}`\n\n**Location:** {}:{}-{}\n\n",
        node.name, node.file_path, node.start_line, node.end_line
    );

    // Group by edge kind
    let mut by_kind: std::collections::HashMap<EdgeKind, Vec<_>> = std::collections::HashMap::new();
    for edge in &edges {
        by_kind.entry(edge.kind).or_default().push(edge);
    }

    let mut total = 0;

    // Process each kind
    for kind in [
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::Extends,
        EdgeKind::Implements,
        EdgeKind::Contains,
        EdgeKind::References,
        EdgeKind::Exports,
    ] {
        if let Some(edges) = by_kind.get(&kind) {
            output.push_str(&format!("### {} ({}):\n\n", kind.as_str(), edges.len()));
            total += edges.len();

            for edge in edges.iter().take(MAX_REFERENCES_PER_KIND) {
                // Get the source node (what is referencing us)
                if let Ok(Some(source)) = db.get_node(edge.source_id) {
                    output.push_str(&format!(
                        "- `{}` ({}) - {}",
                        source.name,
                        source.kind.as_str(),
                        source.file_path
                    ));
                    if let Some(line) = edge.line {
                        output.push_str(&format!(":{}", line));
                    }
                    output.push('\n');
                }
            }

            if edges.len() > MAX_REFERENCES_PER_KIND {
                output.push_str(&format!(
                    "  ... and {} more\n",
                    edges.len() - MAX_REFERENCES_PER_KIND
                ));
            }
            output.push('\n');
        }
    }

    output.push_str(&format!("**Total references:** {}\n", total));

    output
}
