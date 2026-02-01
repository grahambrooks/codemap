# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast type checking
```

## Test Commands

```bash
cargo test               # Run all tests
cargo test --lib         # Run library tests only
cargo test types::       # Run tests in types module
cargo test db::          # Run tests in db module
cargo test extraction::  # Run tests in extraction module
cargo test graph::       # Run tests in graph module
cargo test --test integration_test  # Run integration tests
```

## Lint Commands

```bash
cargo fmt                # Format code
cargo fmt -- --check     # Check formatting without changes
cargo clippy             # Run linter
cargo clippy --fix       # Auto-fix clippy warnings
```

## Run Commands

```bash
cargo run -- index [path]           # Index a codebase
cargo run -- serve                  # Start MCP server (stdio)
cargo run -- serve --port 8080      # Start MCP server (HTTP)
cargo run -- status [path]          # Show index statistics
cargo run -- search <query>         # Search for symbols
cargo run -- context <task>         # Build context for a task
```

## Architecture

codemap is a semantic code intelligence MCP server that builds a knowledge graph of codebases to enhance AI-assisted
code exploration.

### Core Concepts

- **Node**: A code symbol (function, class, method, struct, interface, trait, enum, constant, variable, module, file)
- **Edge**: A relationship between nodes (calls, contains, imports, exports, extends, implements)
- **Knowledge Graph**: The complete set of nodes and edges stored in SQLite

### Module Structure

```
src/
├── main.rs          # CLI entry point (clap-based)
├── lib.rs           # Core indexing logic (index_codebase function)
├── types.rs         # Type definitions (Node, Edge, NodeKind, EdgeKind, Language, etc.)
├── db/mod.rs        # SQLite database operations (CRUD, queries, transactions)
├── extraction/mod.rs # Tree-sitter code extraction (multi-language parsing)
├── graph/mod.rs     # Graph traversal algorithms (callers, callees, impact analysis)
├── context/mod.rs   # Context building for AI tasks
└── mcp/mod.rs       # MCP protocol handlers (7 tools)
```

### Data Flow

1. **Indexing**: `index_codebase()` walks the file tree, uses tree-sitter to parse each supported file, extracts symbols
   and relationships, stores them in SQLite
2. **Reference Resolution**: After initial extraction, unresolved references (function calls, imports) are matched to
   actual node definitions
3. **Querying**: MCP tools query the graph for callers, callees, impact analysis, and context building

### Key Dependencies

- **rmcp**: MCP protocol implementation (stdio and HTTP transports)
- **tree-sitter**: Code parsing with language-specific grammars
- **rusqlite**: SQLite database with bundled SQLite
- **axum**: HTTP server for MCP HTTP transport
- **ignore**: Respects .gitignore when walking directories

### Database Schema

The SQLite database (`.codemap/index.db`) contains:

- **files**: Indexed source files with content hashes for incremental updates
- **nodes**: Code symbols with kind, name, location, visibility
- **edges**: Relationships between nodes
- **unresolved_refs**: References pending resolution

### Supported Languages

Rust, TypeScript, JavaScript, Python, Go, Java, C, C++

### MCP Tools

| Tool              | Description                                    |
|-------------------|------------------------------------------------|
| `codemap_context` | Build focused code context for a specific task |
| `codemap_search`  | Quick symbol search by name                    |
| `codemap_callers` | Find all callers of a symbol                   |
| `codemap_callees` | Find all callees of a symbol                   |
| `codemap_impact`  | Analyze the impact radius of changes           |
| `codemap_node`    | Get detailed symbol information                |
| `codemap_status`  | Get index statistics                           |
