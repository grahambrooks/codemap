# codemap

[![CI](https://github.com/grahambrooks/codemap/actions/workflows/ci.yml/badge.svg)](https://github.com/grahambrooks/codemap/actions/workflows/ci.yml)
[![Release](https://github.com/grahambrooks/codemap/actions/workflows/release.yml/badge.svg)](https://github.com/grahambrooks/codemap/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://modelcontextprotocol.io)

Semantic code intelligence MCP server - build knowledge graphs of codebases to enhance AI-assisted code exploration.

codemap is a rust implementation of https://github.com/colbymchenry/codegraph. Why? Ongoing exploration of compiled
binary deployment of MCP Servers.

## Features

- **Multi-language support**: Rust, TypeScript, JavaScript, Python, Go, Java, C, C++
- **Symbol extraction**: functions, classes, methods, structs, interfaces, traits, enums, constants
- **Relationship tracking**: calls, contains, imports, exports, extends, implements
- **Impact analysis**: trace the effect of changes through the codebase
- **Advanced code intelligence**:
  - Find call paths between functions
  - Detect unused/dead code
  - Explore class hierarchies
  - Locate all interface implementations
  - Analyze change impact by line range
- **Incremental indexing**: only re-indexes changed files using content hashing
- **Dual transport**: stdio (default) and HTTP server modes

## Installation

### Claude Desktop (Recommended)

Download and install the MCPB bundle for your platform:

| Platform              | Download                                                                                   |
|-----------------------|--------------------------------------------------------------------------------------------|
| macOS (Apple Silicon) | [codemap-x.x.x-darwin-arm64.mcpb](https://github.com/grahambrooks/codemap/releases/latest) |
| macOS (Intel)         | [codemap-x.x.x-darwin-x64.mcpb](https://github.com/grahambrooks/codemap/releases/latest)   |
| Windows               | [codemap-x.x.x-windows-x64.mcpb](https://github.com/grahambrooks/codemap/releases/latest)  |
| Linux                 | [codemap-x.x.x-linux-x64.mcpb](https://github.com/grahambrooks/codemap/releases/latest)    |

1. Download the `.mcpb` file for your platform from [Releases](https://github.com/grahambrooks/codemap/releases/latest)
2. Open Claude Desktop
3. Drag and drop the `.mcpb` file onto Claude Desktop, or use **File > Install MCP Server**
4. Configure the project root when prompted

### Standalone Binary

Download the pre-built binary from [Releases](https://github.com/grahambrooks/codemap/releases/latest):

| Platform              | File                                  |
|-----------------------|---------------------------------------|
| macOS (Apple Silicon) | `codemap-VERSION-darwin-arm64.tar.gz` |
| macOS (Intel)         | `codemap-VERSION-darwin-x64.tar.gz`   |
| Linux x64             | `codemap-VERSION-linux-x64.tar.gz`    |
| Windows x64           | `codemap-VERSION-windows-x64.zip`     |

```bash
# Example: macOS (Apple Silicon)
tar xzf codemap-0.1.0-darwin-arm64.tar.gz
sudo mv codemap /usr/local/bin/

# Example: Linux
tar xzf codemap-0.1.0-linux-x64.tar.gz
sudo mv codemap /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/grahambrooks/codemap
cd codemap
cargo build --release
sudo cp target/release/codemap /usr/local/bin/
```

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

### Core Tools

| Tool                     | Description                                              |
|--------------------------|----------------------------------------------------------|
| `codemap-context`        | Build focused code context for a specific task           |
| `codemap-search`         | Quick symbol search by name                              |
| `codemap-callers`        | Find all callers of a symbol                             |
| `codemap-callees`        | Find all callees of a symbol                             |
| `codemap-impact`         | Analyze the impact radius of changes                     |
| `codemap-node`           | Get detailed symbol information                          |
| `codemap-definition`     | Get the full source code of a symbol with context        |
| `codemap-file`           | List all symbols defined in a specific file              |
| `codemap-references`     | Find all references to a symbol                          |
| `codemap-reindex`        | Trigger incremental reindexing of changed files          |
| `codemap-status`         | Get index statistics                                     |

### Advanced Tools

| Tool                     | Description                                              |
|--------------------------|----------------------------------------------------------|
| `codemap-hierarchy`      | Get class/module hierarchy (parent/child relationships)  |
| `codemap-path`           | Find call paths between two symbols                      |
| `codemap-unused`         | Find unused/dead code with no incoming references        |
| `codemap-implementations`| Find all implementations of an interface/trait           |
| `codemap-diff-impact`    | Analyze the impact of changing a specific code region    |

### Example Use Cases

**Find dead code for cleanup:**
```
Use codemap-unused to find all unused functions and classes
```

**Understand function call chains:**
```
Use codemap-path with from="main" and to="database_query" to see how data flows
```

**Assess change impact:**
```
Use codemap-diff-impact with file_path="src/auth.rs" start_line=45 end_line=60
to see what would be affected by changes in that region
```

**Explore OOP hierarchies:**
```
Use codemap-hierarchy with symbol="BaseHandler" to see all parent/child relationships
```

**Find all trait implementations:**
```
Use codemap-implementations with symbol="Iterator" to find all structs implementing Iterator
```

## Configuration

### Claude Desktop (MCPB)

When installed via MCPB bundle, codemap is automatically configured. You can set the project root in Claude Desktop's
MCP server settings.

### Claude Desktop (Manual)

For standalone binary installations, add to your Claude Desktop configuration (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "codemap": {
      "command": "/usr/local/bin/codemap",
      "args": [
        "serve"
      ],
      "env": {
        "CODEMAP_ROOT": "/path/to/your/project"
      }
    }
  }
}
```

### GitHub Copilot

Configure codemap as an MCP server for GitHub Copilot in VS Code or the CLI.

**VS Code** - Add to your VS Code settings (`settings.json`):

```json
{
  "github.copilot.chat.mcp.servers": {
    "codemap": {
      "command": "/usr/local/bin/codemap",
      "args": [
        "serve"
      ],
      "env": {
        "CODEMAP_ROOT": "${workspaceFolder}"
      }
    }
  }
}
```

**Repository Config** - Add `.copilot/mcp.json` to your repository:

```json
{
  "mcpServers": {
    "codemap": {
      "command": "codemap",
      "args": [
        "serve"
      ],
      "env": {
        "CODEMAP_ROOT": "."
      }
    }
  }
}
```

**HTTP Mode** - For remote MCP server support:

```json
{
  "mcpServers": {
    "codemap": {
      "type": "http",
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

See [GitHub Copilot MCP documentation](https://docs.github.com/copilot/customizing-copilot/using-model-context-protocol/extending-copilot-chat-with-mcp)
for more details.

### OpenAI Codex

Configure codemap for OpenAI Codex CLI or VS Code extension. Codex uses TOML configuration at `~/.codex/config.toml`.

**Add via CLI:**

```bash
codex mcp add codemap --command "/usr/local/bin/codemap" --args "serve"
```

**Manual Configuration** - Add to `~/.codex/config.toml`:

```toml
[mcp_servers.codemap]
command = "/usr/local/bin/codemap"
args = ["serve"]

[mcp_servers.codemap.env]
CODEMAP_ROOT = "/path/to/your/project"
```

**HTTP Mode:**

```toml
[mcp_servers.codemap]
type = "url"
url = "http://localhost:8080/mcp"
```

See [OpenAI Codex MCP documentation](https://developers.openai.com/codex/mcp/) for more details.

### Environment Variables

| Variable       | Description            | Default           |
|----------------|------------------------|-------------------|
| `CODEMAP_ROOT` | Project root directory | Current directory |

### First-Time Setup

Before using codemap, index your project:

```bash
cd /path/to/your/project
codemap index
```

This creates a `.codemap/` directory with the SQLite index. Re-run `codemap index` after significant code changes to
update the index.

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

| Module       | Description                                                |
|--------------|------------------------------------------------------------|
| `types`      | Core type definitions (NodeKind, EdgeKind, Language, etc.) |
| `db`         | SQLite database schema and operations                      |
| `extraction` | Tree-sitter based code parsing and symbol extraction       |
| `graph`      | Graph algorithms (callers, callees, impact analysis)       |
| `context`    | Context builder for AI task assistance                     |
| `mcp`        | MCP protocol server implementation                         |

## License

MIT
