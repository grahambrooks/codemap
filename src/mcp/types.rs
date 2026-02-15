//! Request and response types for MCP tools

use rmcp::schemars;
use serde::Deserialize;

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
