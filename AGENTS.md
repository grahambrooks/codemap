# Agent Development Guide

This file provides guidance to AI coding agents (Claude, GitHub Copilot, etc.) when working in this repository.
It is optimized for quick reference and efficient agent-assisted development.

## Quick Reference

### Development Workflow

| Task | Command | Notes |
|------|---------|-------|
| **Build** | `cargo build` | Debug build, fast compilation |
| **Build Release** | `cargo build --release` | Optimized for production |
| **Quick Check** | `cargo check` | Fast type checking without codegen |
| **Run Tests** | `cargo test` | All tests (unit + integration) |
| **Format Code** | `cargo fmt` | Auto-format before commit |
| **Lint** | `cargo clippy` | Catch common mistakes |
| **Fix Lints** | `cargo clippy --fix` | Auto-fix safe warnings |

### Application Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `cargo run -- index <path>` | Index a codebase | `cargo run -- index .` |
| `cargo run -- serve` | Start MCP server (stdio) | For IDE integration |
| `cargo run -- serve --port 8080` | Start HTTP server | For web clients |
| `cargo run -- status [path]` | Show index stats | Quick health check |
| `cargo run -- search <query>` | Search symbols | `cargo run -- search Database` |
| `cargo run -- context <task>` | Build AI context | Experimental feature |

### Targeted Testing

```bash
cargo test --lib                    # Library tests only (fast)
cargo test types::                  # Types module tests
cargo test db::                     # Database tests
cargo test extraction::             # Code extraction tests
cargo test graph::                  # Graph algorithm tests
cargo test --test integration_test  # End-to-end tests
```

## Project Overview

**codemap** is a semantic code intelligence MCP (Model Context Protocol) server that builds a knowledge graph of codebases for AI-assisted development.

### Core Concepts

| Concept | Description | Examples |
|---------|-------------|----------|
| **Node** | Code symbol in the graph | function, class, method, struct, interface, trait, enum, constant, variable, module, file |
| **Edge** | Relationship between nodes | calls, contains, imports, exports, extends, implements |
| **Knowledge Graph** | Complete symbol + relationship data | Stored in SQLite (`.codemap/index.db`) |

### Key Value Propositions

1. **Fast Symbol Search**: Find any function/class across large codebases in milliseconds
2. **Impact Analysis**: Understand what breaks when you change code
3. **Call Graph Navigation**: Follow function calls up and down the stack
4. **AI Context Building**: Generate focused context for LLM coding tasks

### Module Structure

| Module | Responsibility | Key Functions |
|--------|----------------|---------------|
| `main.rs` | CLI entry point | `main()`, `run_server()`, command parsing |
| `lib.rs` | Core indexing | `index_codebase()`, file walking, orchestration |
| `types.rs` | Type system | `Node`, `Edge`, `Language`, `Visibility`, enums |
| `db/mod.rs` | Database layer | CRUD operations, queries, transactions |
| `extraction/mod.rs` | Code parsing | Tree-sitter extraction, symbol detection |
| `graph/mod.rs` | Graph algorithms | `find_callers()`, `find_callees()`, impact analysis |
| `context/mod.rs` | AI context | Task-focused code context building |
| `mcp/mod.rs` | MCP protocol | Tool definitions, handler orchestration |
| `mcp/handlers/*` | Tool handlers | Individual tool implementations |

**File Tree**:
```
src/
├── main.rs          # CLI with clap
├── lib.rs           # Core indexing logic
├── types.rs         # All data types
├── db/mod.rs        # SQLite operations
├── extraction/
│   ├── mod.rs       # Tree-sitter extraction
│   └── languages.rs # Language configs
├── graph/mod.rs     # Graph traversal
├── context/mod.rs   # Context building
└── mcp/
    ├── mod.rs            # Tool definitions (~220 lines)
    ├── types.rs          # Request types
    ├── constants.rs      # Configuration
    ├── format.rs         # Shared formatting
    └── handlers/
        ├── mod.rs
        ├── context.rs    # Context tool
        ├── search.rs     # Search tool
        ├── graph.rs      # Callers/callees/impact
        ├── symbol.rs     # Node/definition/references
        ├── file.rs       # File listing
        ├── reindex.rs    # Reindexing
        └── status.rs     # Status reporting
```

### Data Flow

```
┌─────────────────┐
│  Source Files   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     tree-sitter parses each file
│  Extraction     │ ──► Identifies symbols (functions, classes, etc.)
│  (extraction/)  │     Creates edges (calls, contains, imports)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Database       │     Stores nodes & edges in SQLite
│  (db/)          │     Handles transactions & queries
└────────┬────────┘
         │
         ▼
┌─────────────────┐     Resolves function calls to definitions
│  Resolution     │     Matches imports to actual symbols
│  (db/)          │     Links references across files
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Graph Queries  │     Callers/callees traversal
│  (graph/)       │     Impact analysis
└────────┬────────┘     Symbol search
         │
         ▼
┌─────────────────┐
│  MCP Tools      │     Serve results to IDE/editor
│  (mcp/)         │     Via stdio or HTTP
└─────────────────┘
```

**Step-by-Step**:
1. **Index**: Walk files → Parse with tree-sitter → Extract symbols/relationships → Store in SQLite
2. **Resolve**: Match unresolved references (calls, imports) to actual symbol definitions
3. **Query**: Graph algorithms traverse edges to answer "who calls this?" or "what does this call?"
4. **Serve**: MCP protocol exposes queries as tools for AI assistants

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `rmcp` | MCP protocol (stdio + HTTP) |
| `tree-sitter` | Code parsing (50+ languages) |
| `rusqlite` | SQLite database |
| `axum` | HTTP server |
| `ignore` | .gitignore-aware file walking |
| `clap` | CLI parsing |
| `tracing` | Structured logging |

### Database Schema

**Location**: `.codemap/index.db` (SQLite)

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `files` | Indexed source files | `path`, `content_hash`, `language`, `indexed_at` |
| `nodes` | Code symbols | `id`, `kind`, `name`, `file_path`, `start_line`, `end_line`, `language`, `visibility` |
| `edges` | Relationships | `id`, `source_id`, `target_id`, `kind`, `file_path`, `line` |
| `unresolved_refs` | Pending references | `source_node_id`, `reference_name`, `kind` |

**Indexes**:
- `idx_nodes_name` on `nodes(name)` - Fast symbol search
- `idx_edges_source` on `edges(source_id)` - Fast callees lookup
- `idx_edges_target` on `edges(target_id)` - Fast callers lookup
- `idx_files_path` on `files(path)` - Fast file lookup

**Note**: Content hashes enable incremental re-indexing (only changed files are re-parsed)

### Supported Languages

Rust, TypeScript/JavaScript, Python, Go, Java, C, C++

**Adding Languages**: Edit `src/extraction/languages.rs` with tree-sitter grammar config

### MCP Tools

| Tool | Input | Use Case |
|------|-------|----------|
| `codemap-context` | `{task: string}` | AI coding tasks, feature planning |
| `codemap-search` | `{query: string}` | Find matching symbols |
| `codemap-callers` | `{symbol: string}` | Who calls this function? |
| `codemap-callees` | `{symbol: string}` | What does this call? |
| `codemap-impact` | `{symbol: string}` | What breaks if changed? |
| `codemap-definition` | `{symbol: string, context_lines?: u32}` | View source code |
| `codemap-file` | `{path: string}` | List symbols in file |
| `codemap-references` | `{symbol: string}` | All usages of symbol |
| `codemap-node` | `{symbol: string}` | Symbol metadata |
| `codemap-status` | None | Index health check |
| `codemap-reindex` | `{files?: string[]}` | Refresh index |

## Agent-Specific Guidelines

### For Making Changes

1. **Always run tests first** to establish baseline: `cargo test`
2. **Format before committing**: `cargo fmt`
3. **Fix lints**: `cargo clippy --fix`
4. **Run tests again** to verify no breakage: `cargo test`
5. **Update this file** if adding new features or changing architecture

### Code Style

- **Prefer descriptive names**: `extract_visibility()` not `get_vis()`
- **Keep functions small**: < 50 lines ideal, < 100 lines maximum
- **Document public APIs**: Use `///` doc comments for public items
- **Use `Result<T>` for errors**: Avoid panics except in tests/assertions
- **Prefer iterators**: Over manual loops for collections
- **Use `?` operator**: For error propagation in functions returning `Result`

### Modularity Best Practices

**When to Split Modules**:
- File exceeds 500 lines → Extract related functions to submodules
- Repeated code patterns → Create shared utilities module
- Multiple responsibilities → Separate by concern (handlers/, types.rs, constants.rs)

**Module Organization Pattern**:
```
module/
├── mod.rs          # Public API, re-exports, orchestration
├── types.rs        # Data structures, request/response types
├── constants.rs    # Configuration values
├── format.rs       # Shared formatting/utilities
└── handlers/       # Individual implementations
```

**Key Principles**:
- Each file has one clear responsibility
- Shared logic goes in utilities, not duplicated
- Constants centralized for easy configuration
- Handler functions take dependencies as parameters (testable)

### Testing Strategy

| Test Type | Location | Purpose |
|-----------|----------|---------|
| Unit tests | Same file as code (`#[cfg(test)]`) | Test individual functions |
| Integration tests | `tests/integration_test.rs` | Test end-to-end workflows |
| Module tests | `src/db/mod.rs`, etc. | Test module-level behavior |

**Test Naming**: `test_<function>_<scenario>` (e.g., `test_extract_visibility_rust_private`)

### Common Tasks

#### Adding a New Language

1. Add tree-sitter grammar to `Cargo.toml`
2. Add language variant to `Language` enum in `types.rs`
3. Add config to `src/extraction/languages.rs`
4. Add test in `src/extraction/mod.rs`
5. Update this file's language table

#### Adding a New MCP Tool

1. **Add request type** to `src/mcp/types.rs`:
   ```rust
   #[derive(Debug, Deserialize, schemars::JsonSchema)]
   pub struct MyToolRequest {
       #[schemars(description = "Description")]
       pub param: String,
   }
   ```

2. **Create handler** in `src/mcp/handlers/mytool.rs`:
   ```rust
   use crate::db::Database;
   use crate::mcp::types::MyToolRequest;

   pub fn handle_mytool(db: &Database, req: &MyToolRequest) -> String {
       // Implementation
   }
   ```

3. **Add to handlers/mod.rs**: `pub mod mytool;`

4. **Add tool definition** to `src/mcp/mod.rs`:
   ```rust
   #[tool(name = "codemap-mytool", description = "...")]
   fn codemap_mytool(&self, Parameters(req): Parameters<MyToolRequest>) -> String {
       let db = match self.db.lock() {
           Ok(db) => db,
           Err(e) => return format!("Error: {}", e),
       };
       handlers::mytool::handle_mytool(&db, &req)
   }
   ```

5. **Test** in `tests/integration_test.rs`
6. **Update** MCP tools table in this file

#### Fixing a Bug

1. Add failing test that reproduces the bug
2. Fix the code
3. Verify test passes
4. Check for similar bugs in related code
5. Add regression test if needed

### Performance Tips

- Use indexed columns in queries
- Batch operations in transactions
- Lazy loading, avoid loading all nodes
- Minimize string allocations (`&str` over `String`)
- Reuse tree-sitter parser

### Debugging Tips

```bash
# Enable debug logging
RUST_LOG=debug cargo run -- index .

# Run single test with output
cargo test test_name -- --nocapture

# Profile indexing performance
cargo build --release
time ./target/release/codemap index large_codebase/

# Inspect database
sqlite3 .codemap/index.db "SELECT COUNT(*) FROM nodes"
sqlite3 .codemap/index.db "SELECT * FROM nodes WHERE name = 'my_function'"
```

### Version Bump

1. Update `Cargo.toml`
2. Run `cargo build --release && cargo test --release`
3. Commit: `git commit -m "chore: bump to vX.Y.Z"`
4. Tag: `git tag vX.Y.Z && git push origin main --tags`

## Quick Problem Solving

| Problem | Solution |
|---------|----------|
| Tests failing | `cargo clean && cargo test` |
| Build errors after merge | `cargo update && cargo build` |
| Clippy warnings | `cargo clippy --fix` |
| Format issues | `cargo fmt` |
| Database locked | Kill server: `pkill codemap` or restart |
| Slow indexing | Check for .gitignore issues, reduce file count |
| Missing language | Add to `Language::from_extension()` in `types.rs` |
| Query timeout | Add database index or optimize query |

## Additional Resources

- **MCP Protocol**: https://modelcontextprotocol.io/
- **Tree-sitter**: https://tree-sitter.github.io/tree-sitter/
- **Rust Book**: https://doc.rust-lang.org/book/
- **SQLite Docs**: https://www.sqlite.org/docs.html
