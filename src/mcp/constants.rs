//! Configuration constants for MCP tools

/// Default maximum number of search results
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// Default maximum number of callers/callees to return
pub const DEFAULT_GRAPH_LIMIT: usize = 20;

/// Default maximum number of context nodes
pub const DEFAULT_CONTEXT_MAX_NODES: usize = 20;

/// Default number of context lines before/after a definition
pub const DEFAULT_CONTEXT_LINES: usize = 3;

/// Default impact analysis depth
pub const DEFAULT_IMPACT_DEPTH: usize = 2;

/// Maximum number of references to show per category
pub const MAX_REFERENCES_PER_KIND: usize = 20;
