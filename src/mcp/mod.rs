//! MCP (Model Context Protocol) server implementation
//!
//! Exposes the code graph functionality as MCP tools:
//! - codemap-context: Build task-specific code context
//! - codemap-search: Find symbols by name
//! - codemap-callers: Find all callers of a symbol
//! - codemap-callees: Find all callees of a symbol
//! - codemap-impact: Analyze change impact
//! - codemap-node: Get detailed symbol information
//! - codemap-status: Get index statistics
//! - codemap-definition: Get source code of a symbol
//! - codemap-file: List all symbols in a file
//! - codemap-references: Find all references to a symbol
//! - codemap-reindex: Trigger incremental reindexing

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;

use crate::context::{format_context_markdown, ContextBuilder, ContextOptions};
use crate::db::Database;
use crate::graph::Graph;
use crate::types::EdgeKind;
use crate::{index_codebase, IndexConfig};

/// Request for context tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ContextRequest {
    #[schemars(description = "Description of the task, bug, or feature to explore")]
    pub task: String,
}

/// Request for search tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    #[schemars(description = "Symbol name or partial name to search for")]
    pub query: String,
}

/// Request for symbol-based tools (callers, callees, impact, node)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SymbolRequest {
    #[schemars(description = "Function/method/class name")]
    pub symbol: String,
}

/// Request for file-based tools
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileRequest {
    #[schemars(description = "File path relative to project root (e.g., 'src/main.rs')")]
    pub path: String,
}

/// Request for definition tool with context options
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DefinitionRequest {
    #[schemars(description = "Function/method/class name")]
    pub symbol: String,
    #[schemars(description = "Number of context lines before/after (default: 3)")]
    pub context_lines: Option<u32>,
}

/// Request for reindex tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReindexRequest {
    #[schemars(
        description = "Optional: specific files to reindex. If empty, reindexes all changed files."
    )]
    pub files: Option<Vec<String>>,
}

/// MCP server handler for codemap
#[derive(Clone)]
pub struct CodeMapHandler {
    tool_router: ToolRouter<Self>,
    db: Arc<Mutex<Database>>,
    project_root: String,
}

#[tool_router]
impl CodeMapHandler {
    pub fn new(db: Database, project_root: String) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db: Arc::new(Mutex::new(db)),
            project_root,
        }
    }

    /// Create a handler with a pre-wrapped database (for sharing across HTTP sessions)
    pub fn new_shared(db: Arc<Mutex<Database>>, project_root: String) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db,
            project_root,
        }
    }

    /// Build focused context for a specific task
    #[tool(
        name = "codemap-context",
        description = "Build focused code context for a specific task. Returns entry points, related symbols, and code snippets."
    )]
    fn codemap_context(&self, Parameters(req): Parameters<ContextRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let builder = ContextBuilder::new(&db, self.project_root.clone());
        let options = ContextOptions {
            max_nodes: 20,
            include_code: true,
            ..Default::default()
        };

        match builder.build_context(&req.task, &options) {
            Ok(context) => format_context_markdown(&context),
            Err(e) => format!("Error: {}", e),
        }
    }

    /// Quick symbol search by name
    #[tool(
        name = "codemap-search",
        description = "Quick symbol search by name. Returns locations only (no code)."
    )]
    fn codemap_search(&self, Parameters(req): Parameters<SearchRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let results = match db.search_nodes(&req.query, None, 10) {
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
            output.push_str(&format!(
                "- **{}** `{}` - {}:{}-{}\n",
                node.kind.as_str(),
                node.name,
                node.file_path,
                node.start_line,
                node.end_line
            ));
            if let Some(ref sig) = node.signature {
                output.push_str(&format!("  `{}`\n", sig));
            }
        }

        output
    }

    /// Find all callers of a symbol
    #[tool(
        name = "codemap-callers",
        description = "Find all functions/methods that call a specific symbol."
    )]
    fn codemap_callers(&self, Parameters(req): Parameters<SymbolRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let graph = Graph::new(&db);
        let callers = match graph.find_callers(&req.symbol, 20) {
            Ok(c) => c,
            Err(e) => return format!("Error: {}", e),
        };

        if callers.is_empty() {
            return format!("No callers found for '{}'", req.symbol);
        }

        let mut output = format!("Found {} callers of '{}':\n\n", callers.len(), req.symbol);

        for caller in callers {
            output.push_str(&format!(
                "- **{}** `{}` - {}:{}\n",
                caller.kind.as_str(),
                caller.name,
                caller.file_path,
                caller.start_line
            ));
        }

        output
    }

    /// Find all callees of a symbol
    #[tool(
        name = "codemap-callees",
        description = "Find all functions/methods that a specific symbol calls."
    )]
    fn codemap_callees(&self, Parameters(req): Parameters<SymbolRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let graph = Graph::new(&db);
        let callees = match graph.find_callees(&req.symbol, 20) {
            Ok(c) => c,
            Err(e) => return format!("Error: {}", e),
        };

        if callees.is_empty() {
            return format!("No callees found for '{}'", req.symbol);
        }

        let mut output = format!("'{}' calls {} functions:\n\n", req.symbol, callees.len());

        for callee in callees {
            output.push_str(&format!(
                "- **{}** `{}` - {}:{}\n",
                callee.kind.as_str(),
                callee.name,
                callee.file_path,
                callee.start_line
            ));
        }

        output
    }

    /// Analyze the impact of changing a symbol
    #[tool(
        name = "codemap-impact",
        description = "Analyze the impact radius of changing a symbol."
    )]
    fn codemap_impact(&self, Parameters(req): Parameters<SymbolRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let graph = Graph::new(&db);
        let analysis = match graph.analyze_impact(&req.symbol, 2) {
            Ok(a) => a,
            Err(e) => return format!("Error: {}", e),
        };

        if analysis.root.is_none() {
            return format!("Symbol '{}' not found", req.symbol);
        }

        let root = analysis.root.unwrap();
        let mut output = format!(
            "## Impact Analysis for `{}`\n\n**Location:** {}:{}-{}\n\n",
            root.name, root.file_path, root.start_line, root.end_line
        );

        output.push_str(&format!(
            "**Total Impact:** {} symbols affected\n\n",
            analysis.total_impact
        ));

        if !analysis.direct_callers.is_empty() {
            output.push_str(&format!(
                "### Direct Callers ({}):\n\n",
                analysis.direct_callers.len()
            ));
            for caller in &analysis.direct_callers {
                output.push_str(&format!(
                    "- `{}` ({}:{}) - {}\n",
                    caller.name,
                    caller.file_path,
                    caller.start_line,
                    caller.kind.as_str()
                ));
            }
        }

        if !analysis.indirect_callers.is_empty() {
            output.push_str(&format!(
                "\n### Indirect Callers ({}):\n\n",
                analysis.indirect_callers.len()
            ));
            for caller in analysis.indirect_callers.iter().take(20) {
                output.push_str(&format!(
                    "- `{}` ({}:{}) - {}\n",
                    caller.name,
                    caller.file_path,
                    caller.start_line,
                    caller.kind.as_str()
                ));
            }
        }

        output
    }

    /// Get the full source code definition of a symbol
    #[tool(
        name = "codemap-definition",
        description = "Get the full source code of a symbol. Returns the complete definition with surrounding context lines."
    )]
    fn codemap_definition(&self, Parameters(req): Parameters<DefinitionRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let node = match db.find_node_by_name(&req.symbol) {
            Ok(Some(n)) => n,
            Ok(None) => return format!("Symbol '{}' not found", req.symbol),
            Err(e) => return format!("Error: {}", e),
        };

        let context_lines = req.context_lines.unwrap_or(3) as usize;

        // Read the source file
        let file_path = Path::new(&self.project_root).join(&node.file_path);
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

    /// List all symbols in a specific file
    #[tool(
        name = "codemap-file",
        description = "List all symbols defined in a specific file. Returns functions, classes, methods, etc."
    )]
    fn codemap_file(&self, Parameters(req): Parameters<FileRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        // Normalize the path (remove leading ./ if present)
        let path = req.path.trim_start_matches("./");

        let nodes = match db.get_nodes_by_file(path) {
            Ok(n) => n,
            Err(e) => return format!("Error: {}", e),
        };

        if nodes.is_empty() {
            return format!("No symbols found in '{}'. File may not be indexed.", path);
        }

        let mut output = format!("## Symbols in `{}`\n\n", path);
        output.push_str(&format!("Found {} symbols:\n\n", nodes.len()));

        // Group by kind for better readability
        let mut by_kind: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();
        for node in &nodes {
            by_kind
                .entry(node.kind.as_str().to_string())
                .or_default()
                .push(node);
        }

        // Sort kinds for consistent output
        let mut kinds: Vec<_> = by_kind.keys().cloned().collect();
        kinds.sort();

        for kind in kinds {
            let nodes = &by_kind[&kind];
            output.push_str(&format!("### {} ({}):\n\n", kind, nodes.len()));
            for node in nodes {
                output.push_str(&format!(
                    "- `{}` (lines {}-{})",
                    node.name, node.start_line, node.end_line
                ));
                if let Some(ref sig) = node.signature {
                    output.push_str(&format!(" - `{}`", sig));
                }
                output.push('\n');
            }
            output.push('\n');
        }

        output
    }

    /// Find all references to a symbol
    #[tool(
        name = "codemap-references",
        description = "Find all references to a symbol including calls, imports, type usages, and other relationships."
    )]
    fn codemap_references(&self, Parameters(req): Parameters<SymbolRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

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
        let mut by_kind: std::collections::HashMap<EdgeKind, Vec<_>> =
            std::collections::HashMap::new();
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

                for edge in edges.iter().take(20) {
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

                if edges.len() > 20 {
                    output.push_str(&format!("  ... and {} more\n", edges.len() - 20));
                }
                output.push('\n');
            }
        }

        output.push_str(&format!("**Total references:** {}\n", total));

        output
    }

    /// Trigger incremental reindexing
    #[tool(
        name = "codemap-reindex",
        description = "Trigger incremental reindexing of the codebase. Only changed files are re-parsed."
    )]
    fn codemap_reindex(&self, Parameters(req): Parameters<ReindexRequest>) -> String {
        let mut db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        // If specific files requested, delete and reindex just those
        if let Some(files) = req.files {
            if files.is_empty() {
                return "No files specified. Provide file paths or omit the parameter to reindex all changed files.".to_string();
            }

            let mut errors = Vec::new();

            for file_path in &files {
                // Normalize path
                let path = file_path.trim_start_matches("./");

                // Delete existing data for this file
                if let Err(e) = db.delete_file(path) {
                    errors.push(format!("{}: {}", path, e));
                }
            }

            // Now run full reindex to pick up the deleted files
            drop(db); // Release the lock

            let config = IndexConfig {
                root: self.project_root.clone(),
                ..Default::default()
            };

            let mut db = match self.db.lock() {
                Ok(db) => db,
                Err(e) => return format!("Error: {}", e),
            };

            match index_codebase(&mut db, &config) {
                Ok(stats) => {
                    let mut output = format!(
                        "## Reindex Complete\n\n**Files reindexed:** {}\n**Symbols found:** {}\n**Edges created:** {}\n**References resolved:** {}\n",
                        stats.files, stats.nodes, stats.edges, stats.resolved_refs
                    );
                    if !errors.is_empty() {
                        output.push_str(&format!("\n**Errors:** {}\n", errors.join(", ")));
                    }
                    output
                }
                Err(e) => format!("Reindex failed: {}", e),
            }
        } else {
            // Full incremental reindex
            let config = IndexConfig {
                root: self.project_root.clone(),
                ..Default::default()
            };

            match index_codebase(&mut db, &config) {
                Ok(stats) => {
                    format!(
                        "## Reindex Complete\n\n**Files processed:** {}\n**Files skipped (unchanged):** {}\n**Symbols found:** {}\n**Edges created:** {}\n**References resolved:** {}\n**Errors:** {}\n",
                        stats.files, stats.skipped, stats.nodes, stats.edges, stats.resolved_refs, stats.errors
                    )
                }
                Err(e) => format!("Reindex failed: {}", e),
            }
        }
    }

    /// Get detailed information about a symbol
    #[tool(
        name = "codemap-node",
        description = "Get detailed information about a specific code symbol."
    )]
    fn codemap_node(&self, Parameters(req): Parameters<SymbolRequest>) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

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

    /// Get index statistics
    #[tool(
        name = "codemap-status",
        description = "Get the status of the codemap index. Shows statistics about indexed files, symbols, and relationships."
    )]
    fn codemap_status(&self) -> String {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Error: {}", e),
        };

        let stats = match db.get_stats() {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };

        let mut output = String::from("## codemap Index Status\n\n");

        output.push_str(&format!("**Total Files:** {}\n", stats.total_files));
        output.push_str(&format!("**Total Symbols:** {}\n", stats.total_nodes));
        output.push_str(&format!("**Total Relationships:** {}\n", stats.total_edges));
        output.push_str(&format!(
            "**Database Size:** {:.2} KB\n",
            stats.db_size_bytes as f64 / 1024.0
        ));

        if !stats.languages.is_empty() {
            output.push_str("\n**Languages:**\n");
            for (lang, count) in &stats.languages {
                output.push_str(&format!("- {}: {} symbols\n", lang.as_str(), count));
            }
        }

        if !stats.node_kinds.is_empty() {
            output.push_str("\n**Symbol Types:**\n");
            for (kind, count) in &stats.node_kinds {
                output.push_str(&format!("- {}: {}\n", kind.as_str(), count));
            }
        }

        output
    }
}

#[tool_handler]
impl ServerHandler for CodeMapHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "codemap provides semantic code intelligence for exploring codebases. \
                Use codemap-context to build task-focused context, codemap-search for quick lookups, \
                codemap-callers/callees/impact for understanding code relationships, \
                codemap-definition to view source code, codemap-file to list symbols in a file, \
                codemap-references for all usages of a symbol, and codemap-reindex to refresh after edits."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
