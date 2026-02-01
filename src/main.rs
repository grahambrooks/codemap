//! codemap: Semantic code intelligence MCP server
//!
//! Usage:
//!   codemap serve              Start the MCP server (stdio transport)
//!   codemap serve --port 8080  Start the MCP server (HTTP transport)
//!   codemap index [path]       Index a codebase
//!   codemap status [path]      Show index statistics
//!   codemap search <query>     Search for symbols
//!   codemap context <task>     Build context for a task

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rmcp::{
    transport::stdio,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServiceExt,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use codemap::context::{format_context_markdown, ContextBuilder, ContextOptions};
use codemap::db::Database;
use codemap::mcp::CodeMapHandler;
use codemap::{index_codebase, IndexConfig};

const DB_DIR: &str = ".codemap";
const DB_FILE: &str = "index.db";

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

            if let Some(port) = port {
                run_http_server(port)?;
            } else {
                run_server()?;
            }
        }
        "index" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            run_index(path)?;
        }
        "status" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".");
            run_status(path)?;
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("Usage: codemap search <query>");
                return Ok(());
            }
            let path = ".";
            let query = &args[2];
            run_search(path, query)?;
        }
        "context" => {
            if args.len() < 3 {
                eprintln!("Usage: codemap context <task>");
                return Ok(());
            }
            let path = ".";
            let task = args[2..].join(" ");
            run_context(path, &task)?;
        }
        "help" | "--help" | "-h" => {
            print_usage();
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
    index [path]           Index a codebase (default: current directory)
    status [path]          Show index statistics
    search <query>         Search for symbols by name
    context <task>         Build context for a task description
    help                   Show this help message

EXAMPLES:
    codemap index                    # Index current directory
    codemap index ~/projects/myapp   # Index specific directory
    codemap serve                    # Start MCP server (stdio)
    codemap serve --port 8080        # Start MCP server (HTTP on port 8080)
    codemap search "authenticate"    # Find symbols matching "authenticate"
    codemap context "add user login" # Build context for implementing login
"#
    );
}

fn setup_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
}

fn get_db_path(project_root: &str) -> PathBuf {
    PathBuf::from(project_root).join(DB_DIR).join(DB_FILE)
}

fn ensure_db_dir(project_root: &str) -> Result<()> {
    let dir = PathBuf::from(project_root).join(DB_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

fn run_index(path: &str) -> Result<()> {
    setup_logging();

    let project_root = std::path::Path::new(path)
        .canonicalize()
        .context("Invalid path")?;
    let project_root = project_root.display().to_string();

    ensure_db_dir(&project_root)?;
    let db_path = get_db_path(&project_root);

    info!("Opening database at {}", db_path.display());
    let mut db = Database::open(&db_path)?;

    let config = IndexConfig {
        root: project_root.clone(),
        ..Default::default()
    };

    let stats = index_codebase(&mut db, &config)?;

    println!("\nIndexing complete!");
    println!("  Files indexed: {}", stats.files);
    println!("  Symbols found: {}", stats.nodes);
    println!("  Relationships: {}", stats.edges);
    println!("  Files skipped: {}", stats.skipped);
    println!("  Refs resolved: {}", stats.resolved_refs);
    if stats.errors > 0 {
        println!("  Errors: {}", stats.errors);
    }

    Ok(())
}

fn run_status(path: &str) -> Result<()> {
    let project_root = std::path::Path::new(path)
        .canonicalize()
        .context("Invalid path")?;
    let project_root = project_root.display().to_string();

    let db_path = get_db_path(&project_root);
    if !db_path.exists() {
        println!("No index found at {}", db_path.display());
        println!("Run 'codemap index {}' first.", path);
        return Ok(());
    }

    let db = Database::open(&db_path)?;
    let stats = db.get_stats()?;

    println!("codemap Index Status");
    println!("=====================");
    println!("Database: {}", db_path.display());
    println!("Files: {}", stats.total_files);
    println!("Symbols: {}", stats.total_nodes);
    println!("Relationships: {}", stats.total_edges);
    println!(
        "Size: {:.2} KB",
        stats.db_size_bytes as f64 / 1024.0
    );

    if !stats.languages.is_empty() {
        println!("\nLanguages:");
        for (lang, count) in &stats.languages {
            println!("  {}: {} symbols", lang.as_str(), count);
        }
    }

    if !stats.node_kinds.is_empty() {
        println!("\nSymbol Types:");
        for (kind, count) in &stats.node_kinds {
            println!("  {}: {}", kind.as_str(), count);
        }
    }

    Ok(())
}

fn run_search(path: &str, query: &str) -> Result<()> {
    let project_root = std::path::Path::new(path)
        .canonicalize()
        .context("Invalid path")?;
    let project_root = project_root.display().to_string();

    let db_path = get_db_path(&project_root);
    if !db_path.exists() {
        println!("No index found. Run 'codemap index' first.");
        return Ok(());
    }

    let db = Database::open(&db_path)?;
    let results = db.search_nodes(query, None, 20)?;

    if results.is_empty() {
        println!("No symbols found matching '{}'", query);
        return Ok(());
    }

    println!("Found {} symbols matching '{}':\n", results.len(), query);

    for node in results {
        println!(
            "  {} {} - {}:{}",
            node.kind.as_str(),
            node.name,
            node.file_path,
            node.start_line
        );
        if let Some(ref sig) = node.signature {
            let sig = sig.lines().next().unwrap_or(sig);
            if sig.len() > 80 {
                println!("    {}...", &sig[..80]);
            } else {
                println!("    {}", sig);
            }
        }
    }

    Ok(())
}

fn run_context(path: &str, task: &str) -> Result<()> {
    let project_root = std::path::Path::new(path)
        .canonicalize()
        .context("Invalid path")?;
    let project_root = project_root.display().to_string();

    let db_path = get_db_path(&project_root);
    if !db_path.exists() {
        println!("No index found. Run 'codemap index' first.");
        return Ok(());
    }

    let db = Database::open(&db_path)?;
    let builder = ContextBuilder::new(&db, project_root);

    let options = ContextOptions {
        max_nodes: 20,
        include_code: true,
        max_code_blocks: 5,
        ..Default::default()
    };

    let context = builder.build_context(task, &options)?;
    let markdown = format_context_markdown(&context);

    println!("{}", markdown);

    Ok(())
}

fn get_project_root_and_db() -> Result<(String, Database)> {
    // Get project root from environment or current directory
    let project_root = env::var("CODEMAP_ROOT")
        .or_else(|_| env::current_dir().map(|p| p.display().to_string()))
        .context("Could not determine project root")?;

    let project_root = std::path::Path::new(&project_root)
        .canonicalize()
        .context("Invalid project root")?
        .display()
        .to_string();

    // Open or create database
    ensure_db_dir(&project_root)?;
    let db_path = get_db_path(&project_root);
    info!("Database path: {}", db_path.display());

    let db = Database::open(&db_path)?;

    // Check if index exists
    let stats = db.get_stats()?;
    if stats.total_files == 0 {
        info!("No index found, consider running 'codemap index' first");
    } else {
        info!(
            "Index loaded: {} files, {} symbols",
            stats.total_files, stats.total_nodes
        );
    }

    Ok((project_root, db))
}

#[tokio::main]
async fn run_server() -> Result<()> {
    // For server mode, log to stderr
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("Starting codemap MCP server (stdio)");

    let (project_root, db) = get_project_root_and_db()?;
    info!("Project root: {}", project_root);

    // Create handler and start server
    let handler = CodeMapHandler::new(db, project_root);
    let service = handler.serve(stdio()).await?;

    info!("MCP server running on stdio");
    service.waiting().await?;

    Ok(())
}

#[tokio::main]
async fn run_http_server(port: u16) -> Result<()> {
    use std::sync::Arc;

    // For server mode, log to stderr
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("Starting codemap MCP server (HTTP on port {})", port);

    let (project_root, db) = get_project_root_and_db()?;
    info!("Project root: {}", project_root);

    // Wrap in Arc for sharing across sessions
    let db = Arc::new(std::sync::Mutex::new(db));

    let ct = tokio_util::sync::CancellationToken::new();

    // Create the HTTP service - each session gets a handler with shared db
    let service = StreamableHttpService::new(
        move || Ok(CodeMapHandler::new_shared(db.clone(), project_root.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    // Create axum router with the MCP endpoint
    let router = axum::Router::new().nest_service("/mcp", service);

    let bind_addr = format!("127.0.0.1:{}", port);
    info!("Listening on http://{}/mcp", bind_addr);

    let tcp_listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Shutting down...");
            ct.cancel();
        })
        .await?;

    Ok(())
}
