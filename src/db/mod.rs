//! Database module for codemap
//!
//! Handles SQLite storage for the code graph including:
//! - Schema creation and migrations
//! - Node and edge storage
//! - File tracking
//! - Query operations

mod schema;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use crate::types::{
    Edge, EdgeKind, FileRecord, IndexStats, Language, Node, NodeKind, UnresolvedReference,
    Visibility,
};

/// Database handle for the code graph
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Initialize the database schema
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(schema::SCHEMA)?;
        Ok(())
    }

    // =========================================================================
    // File Operations
    // =========================================================================

    /// Insert or update a file record
    pub fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO files (path, content_hash, language, size, modified_at, indexed_at, node_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(path) DO UPDATE SET
                content_hash = excluded.content_hash,
                language = excluded.language,
                size = excluded.size,
                modified_at = excluded.modified_at,
                indexed_at = excluded.indexed_at,
                node_count = excluded.node_count
            "#,
            params![
                file.path,
                file.content_hash,
                file.language.as_str(),
                file.size as i64,
                file.modified_at,
                file.indexed_at,
                file.node_count as i64,
            ],
        )?;
        Ok(())
    }

    /// Get a file record by path
    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let result = self
            .conn
            .query_row(
                "SELECT path, content_hash, language, size, modified_at, indexed_at, node_count FROM files WHERE path = ?1",
                params![path],
                |row| {
                    Ok(FileRecord {
                        path: row.get(0)?,
                        content_hash: row.get(1)?,
                        language: Language::from_extension(row.get::<_, String>(2)?.as_str()),
                        size: row.get::<_, i64>(3)? as u64,
                        modified_at: row.get(4)?,
                        indexed_at: row.get(5)?,
                        node_count: row.get::<_, i64>(6)? as u32,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    /// Check if a file needs reindexing
    pub fn needs_reindex(&self, path: &str, content_hash: &str) -> Result<bool> {
        match self.get_file(path)? {
            Some(file) => Ok(file.content_hash != content_hash),
            None => Ok(true),
        }
    }

    /// Delete a file and its nodes/edges
    pub fn delete_file(&self, path: &str) -> Result<()> {
        // Delete edges where source or target is in this file
        self.conn.execute(
            "DELETE FROM edges WHERE source_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            params![path],
        )?;
        self.conn.execute(
            "DELETE FROM edges WHERE target_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            params![path],
        )?;
        // Delete nodes
        self.conn
            .execute("DELETE FROM nodes WHERE file_path = ?1", params![path])?;
        // Delete unresolved references
        self.conn.execute(
            "DELETE FROM unresolved_refs WHERE file_path = ?1",
            params![path],
        )?;
        // Delete file record
        self.conn
            .execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(())
    }

    // =========================================================================
    // Node Operations
    // =========================================================================

    /// Insert a node and return its ID
    pub fn insert_node(&self, node: &Node) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO nodes (
                kind, name, qualified_name, file_path, start_line, end_line,
                start_column, end_column, signature, visibility, docstring,
                is_async, is_static, is_exported, language
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                node.kind.as_str(),
                node.name,
                node.qualified_name,
                node.file_path,
                node.start_line as i64,
                node.end_line as i64,
                node.start_column as i64,
                node.end_column as i64,
                node.signature,
                node.visibility.as_str(),
                node.docstring,
                node.is_async,
                node.is_static,
                node.is_exported,
                node.language.as_str(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: i64) -> Result<Option<Node>> {
        let result = self
            .conn
            .query_row("SELECT * FROM nodes WHERE id = ?1", params![id], |row| {
                Self::row_to_node(row)
            })
            .optional()?;
        Ok(result)
    }

    /// Search nodes by name (case-insensitive prefix match)
    pub fn search_nodes(
        &self,
        query: &str,
        kind: Option<NodeKind>,
        limit: u32,
    ) -> Result<Vec<Node>> {
        let pattern = format!("{}%", query.to_lowercase());

        let sql = if kind.is_some() {
            r#"
            SELECT * FROM nodes
            WHERE LOWER(name) LIKE ?1 AND kind = ?2
            ORDER BY LENGTH(name), name
            LIMIT ?3
            "#
        } else {
            r#"
            SELECT * FROM nodes
            WHERE LOWER(name) LIKE ?1
            ORDER BY LENGTH(name), name
            LIMIT ?2
            "#
        };

        let mut stmt = self.conn.prepare(sql)?;
        let mut nodes = Vec::new();

        if let Some(k) = kind {
            let rows = stmt.query_map(params![pattern, k.as_str(), limit as i64], |row| {
                Self::row_to_node(row)
            })?;
            for row in rows {
                nodes.push(row?);
            }
        } else {
            let rows = stmt.query_map(params![pattern, limit as i64], |row| {
                Self::row_to_node(row)
            })?;
            for row in rows {
                nodes.push(row?);
            }
        }

        Ok(nodes)
    }

    /// Get nodes by file path
    pub fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM nodes WHERE file_path = ?1 ORDER BY start_line")?;
        let rows = stmt.query_map(params![file_path], |row| Self::row_to_node(row))?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    /// Find a node by name (exact match)
    pub fn find_node_by_name(&self, name: &str) -> Result<Option<Node>> {
        let result = self
            .conn
            .query_row(
                "SELECT * FROM nodes WHERE name = ?1 LIMIT 1",
                params![name],
                |row| Self::row_to_node(row),
            )
            .optional()?;
        Ok(result)
    }

    fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
        Ok(Node {
            id: row.get(0)?,
            kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap_or(NodeKind::Function),
            name: row.get(2)?,
            qualified_name: row.get(3)?,
            file_path: row.get(4)?,
            start_line: row.get::<_, i64>(5)? as u32,
            end_line: row.get::<_, i64>(6)? as u32,
            start_column: row.get::<_, i64>(7)? as u32,
            end_column: row.get::<_, i64>(8)? as u32,
            signature: row.get(9)?,
            visibility: Visibility::from_str(&row.get::<_, String>(10).unwrap_or_default()),
            docstring: row.get(11)?,
            is_async: row.get(12)?,
            is_static: row.get(13)?,
            is_exported: row.get(14)?,
            language: Language::from_extension(&row.get::<_, String>(15).unwrap_or_default()),
        })
    }

    // =========================================================================
    // Edge Operations
    // =========================================================================

    /// Insert an edge
    pub fn insert_edge(&self, edge: &Edge) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO edges (source_id, target_id, kind, file_path, line, column)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                edge.source_id,
                edge.target_id,
                edge.kind.as_str(),
                edge.file_path,
                edge.line.map(|l| l as i64),
                edge.column.map(|c| c as i64),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get callers of a node (nodes that call this node)
    pub fn get_callers(&self, node_id: i64, limit: u32) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.* FROM nodes n
            INNER JOIN edges e ON e.source_id = n.id
            WHERE e.target_id = ?1 AND e.kind = 'calls'
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![node_id, limit as i64], |row| Self::row_to_node(row))?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    /// Get callees of a node (nodes that this node calls)
    pub fn get_callees(&self, node_id: i64, limit: u32) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.* FROM nodes n
            INNER JOIN edges e ON e.target_id = n.id
            WHERE e.source_id = ?1 AND e.kind = 'calls'
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![node_id, limit as i64], |row| Self::row_to_node(row))?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    /// Get all edges from a node
    pub fn get_outgoing_edges(&self, node_id: i64) -> Result<Vec<Edge>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM edges WHERE source_id = ?1")?;
        let rows = stmt.query_map(params![node_id], |row| Self::row_to_edge(row))?;

        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    /// Get all edges to a node
    pub fn get_incoming_edges(&self, node_id: i64) -> Result<Vec<Edge>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM edges WHERE target_id = ?1")?;
        let rows = stmt.query_map(params![node_id], |row| Self::row_to_edge(row))?;

        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    fn row_to_edge(row: &rusqlite::Row) -> rusqlite::Result<Edge> {
        Ok(Edge {
            id: row.get(0)?,
            source_id: row.get(1)?,
            target_id: row.get(2)?,
            kind: EdgeKind::from_str(&row.get::<_, String>(3)?).unwrap_or(EdgeKind::References),
            file_path: row.get(4)?,
            line: row.get::<_, Option<i64>>(5)?.map(|l| l as u32),
            column: row.get::<_, Option<i64>>(6)?.map(|c| c as u32),
        })
    }

    // =========================================================================
    // Unresolved References
    // =========================================================================

    /// Insert an unresolved reference
    pub fn insert_unresolved_ref(&self, uref: &UnresolvedReference) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO unresolved_refs (source_node_id, reference_name, kind, file_path, line, column)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                uref.source_node_id,
                uref.reference_name,
                uref.kind.as_str(),
                uref.file_path,
                uref.line as i64,
                uref.column as i64,
            ],
        )?;
        Ok(())
    }

    /// Get all unresolved references
    pub fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedReference>> {
        let mut stmt = self.conn.prepare("SELECT * FROM unresolved_refs")?;
        let rows = stmt.query_map([], |row| {
            Ok(UnresolvedReference {
                source_node_id: row.get(1)?,
                reference_name: row.get(2)?,
                kind: EdgeKind::from_str(&row.get::<_, String>(3)?).unwrap_or(EdgeKind::Calls),
                file_path: row.get(4)?,
                line: row.get::<_, i64>(5)? as u32,
                column: row.get::<_, i64>(6)? as u32,
            })
        })?;

        let mut refs = Vec::new();
        for row in rows {
            refs.push(row?);
        }
        Ok(refs)
    }

    /// Resolve references by matching names to nodes
    pub fn resolve_references(&self) -> Result<u32> {
        let refs = self.get_unresolved_refs()?;
        let mut resolved = 0;

        for uref in refs {
            if let Some(target) = self.find_node_by_name(&uref.reference_name)? {
                let edge = Edge {
                    id: 0,
                    source_id: uref.source_node_id,
                    target_id: target.id,
                    kind: uref.kind,
                    file_path: Some(uref.file_path.clone()),
                    line: Some(uref.line),
                    column: Some(uref.column),
                };
                self.insert_edge(&edge)?;
                resolved += 1;
            }
        }

        // Clear resolved refs
        self.conn.execute("DELETE FROM unresolved_refs", [])?;

        Ok(resolved)
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get index statistics
    pub fn get_stats(&self) -> Result<IndexStats> {
        let total_files: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let total_nodes: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        let total_edges: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;

        // Get database file size
        let db_size_bytes: u64 = self
            .conn
            .query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |row| row.get(0))
            .unwrap_or(0);

        // Get language distribution
        let mut stmt = self
            .conn
            .prepare("SELECT language, COUNT(*) FROM nodes GROUP BY language")?;
        let lang_rows = stmt.query_map([], |row| {
            let lang_str: String = row.get(0)?;
            let count: u64 = row.get(1)?;
            Ok((Language::from_extension(&lang_str), count))
        })?;
        let mut languages = Vec::new();
        for row in lang_rows {
            languages.push(row?);
        }

        // Get node kind distribution
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM nodes GROUP BY kind")?;
        let kind_rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(0)?;
            let count: u64 = row.get(1)?;
            Ok((
                NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
                count,
            ))
        })?;
        let mut node_kinds = Vec::new();
        for row in kind_rows {
            node_kinds.push(row?);
        }

        Ok(IndexStats {
            total_files,
            total_nodes,
            total_edges,
            db_size_bytes,
            languages,
            node_kinds,
        })
    }

    /// Begin a transaction
    pub fn begin_transaction(&mut self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    /// Commit a transaction
    pub fn commit(&mut self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback(&mut self) -> Result<()> {
        self.conn.execute("ROLLBACK", [])?;
        Ok(())
    }
}
