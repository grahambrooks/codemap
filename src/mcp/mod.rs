//! MCP (Model Context Protocol) server implementation
//!
//! Exposes the code graph functionality as MCP tools:
//! - codemap_context: Build task-specific code context
//! - codemap_search: Find symbols by name
//! - codemap_callers: Find all callers of a symbol
//! - codemap_callees: Find all callees of a symbol
//! - codemap_impact: Analyze change impact
//! - codemap_node: Get detailed symbol information
//! - codemap_status: Get index statistics

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
    #[tool(description = "Build focused code context for a specific task. Returns entry points, related symbols, and code snippets.")]
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
    #[tool(description = "Quick symbol search by name. Returns locations only (no code).")]
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

        let mut output = format!("Found {} symbols matching '{}':\n\n", results.len(), req.query);

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
    #[tool(description = "Find all functions/methods that call a specific symbol.")]
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
    #[tool(description = "Find all functions/methods that a specific symbol calls.")]
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
    #[tool(description = "Analyze the impact radius of changing a symbol.")]
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

    /// Get detailed information about a symbol
    #[tool(description = "Get detailed information about a specific code symbol.")]
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
    #[tool(description = "Get the status of the codemap index. Shows statistics about indexed files, symbols, and relationships.")]
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
                Use codemap_context to build task-focused context, codemap_search for quick lookups, \
                and codemap_callers/callees/impact for understanding code relationships."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
