//! Reindexing handler

use crate::db::Database;
use crate::mcp::format::normalize_path;
use crate::mcp::types::ReindexRequest;
use crate::{index_codebase, IndexConfig};

pub fn handle_reindex(db: &mut Database, project_root: &str, req: &ReindexRequest) -> String {
    // If specific files requested, delete and reindex just those
    if let Some(files) = &req.files {
        if files.is_empty() {
            return "No files specified. Provide file paths or omit the parameter to reindex all changed files.".to_string();
        }

        let mut errors = Vec::new();

        for file_path in files {
            // Normalize path
            let path = normalize_path(file_path);

            // Delete existing data for this file
            if let Err(e) = db.delete_file(path) {
                errors.push(format!("{}: {}", path, e));
            }
        }

        // Now run full reindex to pick up the deleted files
        let config = IndexConfig {
            root: project_root.to_string(),
            ..Default::default()
        };

        match index_codebase(db, &config) {
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
            root: project_root.to_string(),
            ..Default::default()
        };

        match index_codebase(db, &config) {
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
