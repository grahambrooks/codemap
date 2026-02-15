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
        // Enable foreign key constraints immediately after opening
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        // Enable foreign key constraints immediately after opening
        conn.execute("PRAGMA foreign_keys = ON", [])?;
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

    /// Insert or update a file record (upsert operation)
    pub fn insert_or_update_file(&self, file: &FileRecord) -> Result<()> {
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
            let rows = stmt.query_map(params![pattern, limit as i64], Self::row_to_node)?;
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
        let rows = stmt.query_map(params![file_path], Self::row_to_node)?;

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
                Self::row_to_node,
            )
            .optional()?;
        Ok(result)
    }

    fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
        Ok(Node {
            id: row.get(0)?,
            kind: NodeKind::parse(&row.get::<_, String>(1)?).unwrap_or(NodeKind::Function),
            name: row.get(2)?,
            qualified_name: row.get(3)?,
            file_path: row.get(4)?,
            start_line: row.get::<_, i64>(5)? as u32,
            end_line: row.get::<_, i64>(6)? as u32,
            start_column: row.get::<_, i64>(7)? as u32,
            end_column: row.get::<_, i64>(8)? as u32,
            signature: row.get(9)?,
            visibility: Visibility::parse(&row.get::<_, String>(10).unwrap_or_default()),
            docstring: row.get(11)?,
            is_async: row.get(12)?,
            is_static: row.get(13)?,
            is_exported: row.get(14)?,
            language: Language::parse(&row.get::<_, String>(15).unwrap_or_default()),
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
        let rows = stmt.query_map(params![node_id, limit as i64], Self::row_to_node)?;

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
        let rows = stmt.query_map(params![node_id, limit as i64], Self::row_to_node)?;

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
        let rows = stmt.query_map(params![node_id], Self::row_to_edge)?;

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
        let rows = stmt.query_map(params![node_id], Self::row_to_edge)?;

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
            kind: EdgeKind::parse(&row.get::<_, String>(3)?).unwrap_or(EdgeKind::References),
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
                kind: EdgeKind::parse(&row.get::<_, String>(3)?).unwrap_or(EdgeKind::Calls),
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
        let total_files: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let total_nodes: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        let total_edges: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;

        // Get database file size
        let db_size_bytes: i64 = self
            .conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Get language distribution
        let mut stmt = self
            .conn
            .prepare("SELECT language, COUNT(*) FROM nodes GROUP BY language")?;
        let lang_rows = stmt.query_map([], |row| {
            let lang_str: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((Language::parse(&lang_str), count as u64))
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
            let count: i64 = row.get(1)?;
            Ok((
                NodeKind::parse(&kind_str).unwrap_or(NodeKind::Function),
                count as u64,
            ))
        })?;
        let mut node_kinds = Vec::new();
        for row in kind_rows {
            node_kinds.push(row?);
        }

        Ok(IndexStats {
            total_files: total_files as u64,
            total_nodes: total_nodes as u64,
            total_edges: total_edges as u64,
            db_size_bytes: db_size_bytes as u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(name: &str, kind: NodeKind, file_path: &str) -> Node {
        Node {
            id: 0,
            kind,
            name: name.to_string(),
            qualified_name: Some(format!("test::{}", name)),
            file_path: file_path.to_string(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 1,
            signature: Some(format!("fn {}()", name)),
            visibility: Visibility::Public,
            docstring: None,
            is_async: false,
            is_static: false,
            is_exported: true,
            language: Language::Rust,
        }
    }

    fn create_test_file(path: &str) -> FileRecord {
        FileRecord {
            path: path.to_string(),
            content_hash: "abc123".to_string(),
            language: Language::Rust,
            size: 1000,
            modified_at: 1234567890,
            indexed_at: 1234567890,
            node_count: 5,
        }
    }

    // Database initialization tests
    #[test]
    fn test_in_memory_database_creation() {
        let db = Database::in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_database_stats_empty() {
        let db = Database::in_memory().unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
    }

    // File operations tests
    #[test]
    fn test_upsert_and_get_file() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");

        db.insert_or_update_file(&file).unwrap();
        let retrieved = db.get_file("test.rs").unwrap();

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.path, "test.rs");
        assert_eq!(retrieved.content_hash, "abc123");
        assert_eq!(retrieved.node_count, 5);
    }

    #[test]
    fn test_file_upsert_updates_existing() {
        let db = Database::in_memory().unwrap();
        let mut file = create_test_file("src/lib.rs");

        db.insert_or_update_file(&file).unwrap();

        file.content_hash = "updated_hash".to_string();
        file.node_count = 10;
        db.insert_or_update_file(&file).unwrap();

        let retrieved = db.get_file("src/lib.rs").unwrap().unwrap();
        assert_eq!(retrieved.content_hash, "updated_hash");
        assert_eq!(retrieved.node_count, 10);
    }

    #[test]
    fn test_get_nonexistent_file() {
        let db = Database::in_memory().unwrap();
        let result = db.get_file("nonexistent.rs").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_needs_reindex_new_file() {
        let db = Database::in_memory().unwrap();
        let needs = db.needs_reindex("new_file.rs", "somehash").unwrap();
        assert!(needs);
    }

    #[test]
    fn test_needs_reindex_unchanged_file() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let needs = db.needs_reindex("test.rs", "abc123").unwrap();
        assert!(!needs);
    }

    #[test]
    fn test_needs_reindex_changed_file() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let needs = db.needs_reindex("test.rs", "different_hash").unwrap();
        assert!(needs);
    }

    // Node operations tests
    #[test]
    fn test_insert_and_get_node() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let node = create_test_node("my_function", NodeKind::Function, "test.rs");
        let id = db.insert_node(&node).unwrap();

        let retrieved = db.get_node(id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "my_function");
        assert_eq!(retrieved.kind, NodeKind::Function);
    }

    #[test]
    fn test_get_nonexistent_node() {
        let db = Database::in_memory().unwrap();
        let result = db.get_node(999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_search_nodes() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.insert_node(&create_test_node("process_data", NodeKind::Function, "test.rs"))
            .unwrap();
        db.insert_node(&create_test_node("process_input", NodeKind::Function, "test.rs"))
            .unwrap();
        db.insert_node(&create_test_node("handle_error", NodeKind::Function, "test.rs"))
            .unwrap();

        let results = db.search_nodes("process", None, 10).unwrap();
        assert_eq!(results.len(), 2);

        let results = db.search_nodes("handle", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "handle_error");
    }

    #[test]
    fn test_search_nodes_with_kind_filter() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.insert_node(&create_test_node("MyClass", NodeKind::Class, "test.rs"))
            .unwrap();
        db.insert_node(&create_test_node("my_function", NodeKind::Function, "test.rs"))
            .unwrap();

        let results = db.search_nodes("my", Some(NodeKind::Function), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, NodeKind::Function);
    }

    #[test]
    fn test_search_nodes_case_insensitive() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.insert_node(&create_test_node("MyFunction", NodeKind::Function, "test.rs"))
            .unwrap();

        let results = db.search_nodes("myfunction", None, 10).unwrap();
        assert_eq!(results.len(), 1);

        let results = db.search_nodes("MYFUNCTION", None, 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_find_node_by_name() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.insert_node(&create_test_node("unique_name", NodeKind::Function, "test.rs"))
            .unwrap();

        let result = db.find_node_by_name("unique_name").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "unique_name");

        let result = db.find_node_by_name("nonexistent").unwrap();
        assert!(result.is_none());
    }

    // Edge operations tests
    #[test]
    fn test_insert_edge() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let id1 = db
            .insert_node(&create_test_node("caller", NodeKind::Function, "test.rs"))
            .unwrap();
        let id2 = db
            .insert_node(&create_test_node("callee", NodeKind::Function, "test.rs"))
            .unwrap();

        let edge = Edge {
            id: 0,
            source_id: id1,
            target_id: id2,
            kind: EdgeKind::Calls,
            file_path: Some("test.rs".to_string()),
            line: Some(5),
            column: Some(10),
        };

        let edge_id = db.insert_edge(&edge).unwrap();
        assert!(edge_id > 0);
    }

    #[test]
    fn test_get_callers_and_callees() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let caller_id = db
            .insert_node(&create_test_node("caller", NodeKind::Function, "test.rs"))
            .unwrap();
        let callee_id = db
            .insert_node(&create_test_node("callee", NodeKind::Function, "test.rs"))
            .unwrap();

        let edge = Edge {
            id: 0,
            source_id: caller_id,
            target_id: callee_id,
            kind: EdgeKind::Calls,
            file_path: None,
            line: None,
            column: None,
        };
        db.insert_edge(&edge).unwrap();

        let callers = db.get_callers(callee_id, 10).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "caller");

        let callees = db.get_callees(caller_id, 10).unwrap();
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "callee");
    }

    #[test]
    fn test_get_outgoing_and_incoming_edges() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let id1 = db
            .insert_node(&create_test_node("node1", NodeKind::Function, "test.rs"))
            .unwrap();
        let id2 = db
            .insert_node(&create_test_node("node2", NodeKind::Function, "test.rs"))
            .unwrap();

        let edge = Edge {
            id: 0,
            source_id: id1,
            target_id: id2,
            kind: EdgeKind::Calls,
            file_path: None,
            line: None,
            column: None,
        };
        db.insert_edge(&edge).unwrap();

        let outgoing = db.get_outgoing_edges(id1).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target_id, id2);

        let incoming = db.get_incoming_edges(id2).unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source_id, id1);
    }

    // Unresolved references tests
    #[test]
    fn test_unresolved_refs() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let node_id = db
            .insert_node(&create_test_node("caller", NodeKind::Function, "test.rs"))
            .unwrap();

        let uref = UnresolvedReference {
            source_node_id: node_id,
            reference_name: "some_function".to_string(),
            kind: EdgeKind::Calls,
            file_path: "src/lib.rs".to_string(),
            line: 5,
            column: 10,
        };

        db.insert_unresolved_ref(&uref).unwrap();

        let refs = db.get_unresolved_refs().unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reference_name, "some_function");
    }

    #[test]
    fn test_resolve_references() {
        let db = Database::in_memory().unwrap();
        let file1 = create_test_file("test.rs");
        db.insert_or_update_file(&file1).unwrap();

        let caller_id = db
            .insert_node(&create_test_node("caller", NodeKind::Function, "test.rs"))
            .unwrap();
        let _callee_id = db
            .insert_node(&create_test_node("target_func", NodeKind::Function, "test.rs"))
            .unwrap();

        let uref = UnresolvedReference {
            source_node_id: caller_id,
            reference_name: "target_func".to_string(),
            kind: EdgeKind::Calls,
            file_path: "test.rs".to_string(),
            line: 5,
            column: 10,
        };
        db.insert_unresolved_ref(&uref).unwrap();

        let resolved = db.resolve_references().unwrap();
        assert_eq!(resolved, 1);

        // Check that the edge was created
        let outgoing = db.get_outgoing_edges(caller_id).unwrap();
        assert_eq!(outgoing.len(), 1);

        // Check that unresolved refs are cleared
        let refs = db.get_unresolved_refs().unwrap();
        assert!(refs.is_empty());
    }

    #[test]
    fn test_stats() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.insert_node(&create_test_node("func1", NodeKind::Function, "test.rs"))
            .unwrap();
        db.insert_node(&create_test_node("func2", NodeKind::Function, "test.rs"))
            .unwrap();
        db.insert_node(&create_test_node("MyClass", NodeKind::Class, "test.rs"))
            .unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_edges, 0);
    }

    #[test]
    fn test_delete_file() {
        let db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        let id1 = db
            .insert_node(&create_test_node("func1", NodeKind::Function, "test.rs"))
            .unwrap();
        let id2 = db
            .insert_node(&create_test_node("func2", NodeKind::Function, "test.rs"))
            .unwrap();

        let edge = Edge {
            id: 0,
            source_id: id1,
            target_id: id2,
            kind: EdgeKind::Calls,
            file_path: Some("test.rs".to_string()),
            line: None,
            column: None,
        };
        db.insert_edge(&edge).unwrap();

        db.delete_file("test.rs").unwrap();

        // File should be gone
        assert!(db.get_file("test.rs").unwrap().is_none());

        // Nodes should be gone
        assert!(db.get_node(id1).unwrap().is_none());
        assert!(db.get_node(id2).unwrap().is_none());

        // Stats should show zeros
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
    }

    // Transaction tests
    #[test]
    fn test_transaction_commit() {
        let mut db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.begin_transaction().unwrap();
        db.insert_node(&create_test_node("func1", NodeKind::Function, "test.rs"))
            .unwrap();
        db.commit().unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_nodes, 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let mut db = Database::in_memory().unwrap();
        let file = create_test_file("test.rs");
        db.insert_or_update_file(&file).unwrap();

        db.begin_transaction().unwrap();
        db.insert_node(&create_test_node("func1", NodeKind::Function, "test.rs"))
            .unwrap();
        db.rollback().unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_nodes, 0);
    }
}

#[cfg(test)]
mod language_tests {
    use super::*;
    use crate::types::FileRecord;

    #[test]
    fn test_language_roundtrip() {
        let db = Database::in_memory().unwrap();

        // First insert a file
        let file = FileRecord {
            path: "test.rs".to_string(),
            content_hash: "abc123".to_string(),
            language: Language::Rust,
            size: 100,
            modified_at: 0,
            indexed_at: 0,
            node_count: 1,
        };
        db.insert_or_update_file(&file).unwrap();

        let node = Node {
            id: 0,
            kind: NodeKind::Function,
            name: "test_func".to_string(),
            qualified_name: None,
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            signature: Some("fn test_func()".to_string()),
            visibility: Visibility::Private,
            docstring: None,
            is_async: false,
            is_static: false,
            is_exported: false,
            language: Language::Rust,
        };

        db.insert_node(&node).unwrap();
        let retrieved = db.find_node_by_name("test_func").unwrap().unwrap();

        assert_eq!(
            retrieved.language,
            Language::Rust,
            "Language should be Rust, got {:?}",
            retrieved.language
        );
        assert_eq!(
            retrieved.visibility,
            Visibility::Private,
            "Visibility should be Private, got {:?}",
            retrieved.visibility
        );
    }
}
