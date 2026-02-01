# codemap

[![CI](https://github.com/yourusername/codemap/actions/workflows/ci.yml/badge.svg)](https://github.com/yourusername/codemap/actions/workflows/ci.yml)
[![Release](https://github.com/yourusername/codemap/actions/workflows/release.yml/badge.svg)](https://github.com/yourusername/codemap/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://modelcontextprotocol.io)

Semantic code intelligence MCP server - build knowledge graphs of codebases to enhance AI-assisted code exploration.

## Features

- **Multi-language support**: Rust, TypeScript, JavaScript, Python, Go, Java, C, C++
- **Symbol extraction**: functions, classes, methods, structs, interfaces, traits, enums, constants
- **Relationship tracking**: calls, contains, imports, exports, extends, implements
- **Impact analysis**: trace the effect of changes through the codebase
- **Incremental indexing**: only re-indexes changed files using content hashing
- **Dual transport**: stdio (default) and HTTP server modes

## Installation

### From Source

```bash
git clone https://github.com/yourusername/codemap
cd codemap
cargo build --release
```

### From MCPB Bundle

Download the latest `.mcpb` bundle from [Releases](https://github.com/yourusername/codemap/releases) and install it in Claude Desktop.

## Usage

### Index a Codebase

```bash
# Index current directory
codemap index

# Index specific directory
codemap index ~/projects/myapp
```

### Start MCP Server

```bash
# Start with stdio transport (for Claude Desktop)
codemap serve

# Start with HTTP transport
codemap serve --port 8080
```

### CLI Commands

```bash
codemap index [path]           # Index a codebase
codemap serve                  # Start MCP server (stdio)
codemap serve --port <PORT>    # Start MCP server (HTTP)
codemap status [path]          # Show index statistics
codemap search <query>         # Search for symbols
codemap context <task>         # Build context for a task
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `codemap_context` | Build focused code context for a specific task |
| `codemap_search` | Quick symbol search by name |
| `codemap_callers` | Find all callers of a symbol |
| `codemap_callees` | Find all callees of a symbol |
| `codemap_impact` | Analyze the impact radius of changes |
| `codemap_node` | Get detailed symbol information |
| `codemap_status` | Get index statistics |

## Configuration

### Claude Desktop

Add to your Claude Desktop configuration (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "codemap": {
      "command": "/path/to/codemap",
      "args": ["serve"],
      "env": {
        "CODEMAP_ROOT": "/path/to/your/project"
      }
    }
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CODEMAP_ROOT` | Project root directory | Current directory |

## Architecture

```
codemap/
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Core indexing logic
│   ├── types.rs         # Type definitions (Node, Edge, etc.)
│   ├── db/              # SQLite database operations
│   ├── extraction/      # Tree-sitter code extraction
│   ├── graph/           # Graph traversal algorithms
│   ├── context/         # Context building for AI tasks
│   └── mcp/             # MCP protocol handlers
└── .codemap/
    └── index.db         # SQLite database (per-project)
```

### Core Concepts

- **Node**: A code symbol (function, class, method, etc.)
- **Edge**: A relationship between nodes (calls, contains, imports, etc.)
- **Knowledge Graph**: The complete set of nodes and edges for a codebase

## Development

### Prerequisites

- Rust 1.70+
- SQLite (bundled via rusqlite)

### Building

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo test           # Run tests
```

### Project Structure

| Module | Description |
|--------|-------------|
| `types` | Core type definitions (NodeKind, EdgeKind, Language, etc.) |
| `db` | SQLite database schema and operations |
| `extraction` | Tree-sitter based code parsing and symbol extraction |
| `graph` | Graph algorithms (callers, callees, impact analysis) |
| `context` | Context builder for AI task assistance |
| `mcp` | MCP protocol server implementation |

## License

MIT
