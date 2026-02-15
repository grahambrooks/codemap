//! codemap: Semantic code intelligence MCP server
//!
//! Usage:
//!   codemap serve              Start the MCP server (stdio transport)
//!   codemap serve --port 8080  Start the MCP server (HTTP transport)
//!   codemap index [path]       Index a codebase
//!   codemap status [path]      Show index statistics
//!   codemap search <query>     Search for symbols
//!   codemap context <task>     Build context for a task

mod server;

use std::env;

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use codemap::cli::{context_command, index_command, search_command, status_command};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "serve" => {
            // Check for --port flag
            let port = args
                .iter()
                .position(|a| a == "--port")
                .and_then(|i| args.get(i + 1))
                .and_then(|p| p.parse::<u16>().ok());

            let in_memory = args.iter().any(|a| a == "--in-memory");

            if let Some(port) = port {
                server::start_http(port, in_memory)?;
            } else {
                server::start_stdio(in_memory)?;
            }
        }
        "index" => {
            setup_logging();
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            index_command(path)?;
        }
        "status" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            status_command(path)?;
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("Usage: codemap search <query>");
                return Ok(());
            }
            let path = ".";
            let query = &args[2];
            search_command(path, query)?;
        }
        "context" => {
            if args.len() < 3 {
                eprintln!("Usage: codemap context <task>");
                return Ok(());
            }
            let path = ".";
            let task = args[2..].join(" ");
            context_command(path, &task)?;
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "--version" | "-V" | "version" => {
            print_version();
        }
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!(
        r#"codemap: Semantic code intelligence MCP server

USAGE:
    codemap <COMMAND> [OPTIONS]

COMMANDS:
    serve                  Start the MCP server (stdio transport)
    serve --port <PORT>    Start the MCP server (HTTP transport)
    serve --in-memory      Use in-memory database (no filesystem writes)
    index [path]           Index a codebase (default: current directory)
    status [path]          Show index statistics
    search <query>         Search for symbols by name
    context <task>         Build context for a task description
    help                   Show this help message

ENVIRONMENT:
    CODEMAP_ROOT           Project root directory (default: current directory)
    CODEMAP_IN_MEMORY=1    Use in-memory database (alternative to --in-memory)

EXAMPLES:
    codemap index                    # Index current directory
    codemap index ~/projects/myapp   # Index specific directory
    codemap serve                    # Start MCP server (stdio)
    codemap serve --port 8080        # Start MCP server (HTTP on port 8080)
    codemap serve --in-memory        # Start MCP server with in-memory database
    codemap search "authenticate"    # Find symbols matching "authenticate"
    codemap context "add user login" # Build context for implementing login
"#
    );
}

fn print_version() {
    println!("codemap {}", env!("CARGO_PKG_VERSION"));
}

fn setup_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
}
