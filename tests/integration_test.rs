//! Integration tests for codemap
//!
//! These tests verify the end-to-end workflow of indexing and querying code.

use codemap::db::Database;
use codemap::extraction::Extractor;
use codemap::graph::Graph;
use codemap::types::{FileRecord, Language, NodeKind};
use tempfile::tempdir;

/// Helper to set up a test database with indexed code
fn setup_indexed_db(code: &str, filename: &str) -> Database {
    let db = Database::in_memory().unwrap();

    // Create file record
    let file = FileRecord {
        path: filename.to_string(),
        content_hash: "test_hash".to_string(),
        language: Language::from_extension(
            std::path::Path::new(filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        ),
        size: code.len() as u64,
        modified_at: 0,
        indexed_at: 0,
        node_count: 0,
    };
    db.upsert_file(&file).unwrap();

    // Extract symbols
    let mut extractor = Extractor::new();
    let result = extractor.extract_file(filename, code);

    // Store nodes with ID mapping
    let mut id_map = std::collections::HashMap::new();
    for mut node in result.nodes {
        let old_id = node.id;
        node.id = 0;
        let new_id = db.insert_node(&node).unwrap();
        id_map.insert(old_id, new_id);
    }

    // Store edges with mapped IDs
    for mut edge in result.edges {
        if let (Some(&new_source), Some(&new_target)) =
            (id_map.get(&edge.source_id), id_map.get(&edge.target_id))
        {
            edge.source_id = new_source;
            edge.target_id = new_target;
            db.insert_edge(&edge).unwrap();
        }
    }

    // Store unresolved references
    for mut uref in result.unresolved_refs {
        if let Some(&new_source) = id_map.get(&uref.source_node_id) {
            uref.source_node_id = new_source;
            db.insert_unresolved_ref(&uref).unwrap();
        }
    }

    // Resolve references
    db.resolve_references().unwrap();

    db
}

#[test]
fn test_end_to_end_rust_indexing() {
    let code = r#"
fn main() {
    helper();
    println!("Hello!");
}

fn helper() {
    utility();
}

fn utility() {
    // Does some work
}
"#;

    let db = setup_indexed_db(code, "main.rs");

    // Verify nodes were created
    let stats = db.get_stats().unwrap();
    assert!(stats.total_nodes >= 4); // file + 3 functions

    // Verify we can search for functions
    let results = db.search_nodes("main", None, 10).unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|n| n.name == "main"));

    // Verify we can find the helper function
    let helper = db.find_node_by_name("helper").unwrap();
    assert!(helper.is_some());
}

#[test]
fn test_end_to_end_call_graph() {
    let code = r#"
fn caller() {
    callee();
}

fn callee() {
    // Implementation
}
"#;

    let db = setup_indexed_db(code, "calls.rs");
    let graph = Graph::new(&db);

    // Find callers of callee
    let callers = graph.find_callers("callee", 10).unwrap();
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].name, "caller");

    // Find callees of caller
    let callees = graph.find_callees("caller", 10).unwrap();
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].name, "callee");
}

#[test]
fn test_end_to_end_impact_analysis() {
    let code = r#"
fn base_function() {
    // Core logic
}

fn direct_user() {
    base_function();
}

fn indirect_user() {
    direct_user();
}
"#;

    let db = setup_indexed_db(code, "impact.rs");
    let graph = Graph::new(&db);

    let analysis = graph.analyze_impact("base_function", 3).unwrap();

    assert!(analysis.root.is_some());
    assert_eq!(analysis.root.as_ref().unwrap().name, "base_function");
    assert!(!analysis.direct_callers.is_empty());
    assert!(analysis.total_impact >= 1);
}

#[test]
fn test_end_to_end_typescript() {
    let code = r#"
interface User {
    name: string;
    age: number;
}

class UserService {
    getUser(id: number): User {
        return { name: "Test", age: 25 };
    }

    saveUser(user: User): void {
        console.log(user);
    }
}

function main(): void {
    const service = new UserService();
    const user = service.getUser(1);
    service.saveUser(user);
}
"#;

    let db = setup_indexed_db(code, "user.ts");

    // Check that interface was extracted
    let results = db.search_nodes("User", None, 10).unwrap();
    assert!(results.iter().any(|n| n.kind == NodeKind::Interface));

    // Check that class was extracted
    let results = db.search_nodes("UserService", None, 10).unwrap();
    assert!(results.iter().any(|n| n.kind == NodeKind::Class));

    // Check that function was extracted
    let main = db.find_node_by_name("main").unwrap();
    assert!(main.is_some());
    // Language may be stored differently, just verify the node exists
    assert_eq!(main.unwrap().kind, NodeKind::Function);
}

#[test]
fn test_end_to_end_python() {
    let code = r#"
class Calculator:
    def __init__(self):
        self.value = 0

    def add(self, x):
        self.value += x
        return self

    def result(self):
        return self.value

def main():
    calc = Calculator()
    calc.add(5).add(3)
    print(calc.result())
"#;

    let db = setup_indexed_db(code, "calc.py");

    // Check class
    let calc = db.find_node_by_name("Calculator").unwrap();
    assert!(calc.is_some());
    assert_eq!(calc.unwrap().kind, NodeKind::Class);

    // Check methods
    let add = db.find_node_by_name("add").unwrap();
    assert!(add.is_some());

    // Check function
    let main = db.find_node_by_name("main").unwrap();
    assert!(main.is_some());
    // Language stored as string, just verify the node exists
    assert_eq!(main.unwrap().kind, NodeKind::Function);
}

#[test]
fn test_database_persistence() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create and populate database
    {
        let db = Database::open(&db_path).unwrap();
        let file = FileRecord {
            path: "test.rs".to_string(),
            content_hash: "abc123".to_string(),
            language: Language::Rust,
            size: 100,
            modified_at: 0,
            indexed_at: 0,
            node_count: 1,
        };
        db.upsert_file(&file).unwrap();

        let mut extractor = Extractor::new();
        let result = extractor.extract_file("test.rs", "fn hello() {}");
        for node in result.nodes {
            db.insert_node(&node).unwrap();
        }
    }

    // Reopen and verify
    {
        let db = Database::open(&db_path).unwrap();
        let stats = db.get_stats().unwrap();
        assert!(stats.total_files >= 1);
        assert!(stats.total_nodes >= 1);

        let file = db.get_file("test.rs").unwrap();
        assert!(file.is_some());
    }
}

#[test]
fn test_incremental_indexing() {
    let db = Database::in_memory().unwrap();

    // First index
    let file1 = FileRecord {
        path: "module.rs".to_string(),
        content_hash: "hash_v1".to_string(),
        language: Language::Rust,
        size: 100,
        modified_at: 0,
        indexed_at: 0,
        node_count: 0,
    };
    db.upsert_file(&file1).unwrap();

    // Check that file doesn't need reindexing with same hash
    assert!(!db.needs_reindex("module.rs", "hash_v1").unwrap());

    // Check that file needs reindexing with different hash
    assert!(db.needs_reindex("module.rs", "hash_v2").unwrap());

    // Check that new file needs indexing
    assert!(db.needs_reindex("new_file.rs", "any_hash").unwrap());
}

#[test]
fn test_multi_file_references() {
    let db = Database::in_memory().unwrap();

    // Set up two files
    for path in ["file1.rs", "file2.rs"] {
        let file = FileRecord {
            path: path.to_string(),
            content_hash: "hash".to_string(),
            language: Language::Rust,
            size: 100,
            modified_at: 0,
            indexed_at: 0,
            node_count: 0,
        };
        db.upsert_file(&file).unwrap();
    }

    // Extract and insert nodes from both files
    let mut extractor = Extractor::new();

    let result1 = extractor.extract_file("file1.rs", "fn shared_helper() {}");
    let mut id_map1 = std::collections::HashMap::new();
    for mut node in result1.nodes {
        let old_id = node.id;
        node.id = 0;
        let new_id = db.insert_node(&node).unwrap();
        id_map1.insert(old_id, new_id);
    }

    let result2 = extractor.extract_file("file2.rs", "fn caller() { shared_helper(); }");
    let mut id_map2 = std::collections::HashMap::new();
    for mut node in result2.nodes {
        let old_id = node.id;
        node.id = 0;
        let new_id = db.insert_node(&node).unwrap();
        id_map2.insert(old_id, new_id);
    }

    // Insert unresolved references
    for mut uref in result2.unresolved_refs {
        if let Some(&new_source) = id_map2.get(&uref.source_node_id) {
            uref.source_node_id = new_source;
            db.insert_unresolved_ref(&uref).unwrap();
        }
    }

    // Resolve cross-file references
    let resolved = db.resolve_references().unwrap();
    assert!(resolved >= 1);

    // Verify the cross-file call was resolved
    let graph = Graph::new(&db);
    let callers = graph.find_callers("shared_helper", 10).unwrap();
    assert!(!callers.is_empty());
}

#[test]
fn test_contains_relationship() {
    let code = r#"
mod outer {
    fn inner() {}
}
"#;

    let db = setup_indexed_db(code, "nested.rs");

    // Should have contains edges
    let stats = db.get_stats().unwrap();
    assert!(stats.total_edges > 0);
}

#[test]
fn test_search_with_limit() {
    let db = Database::in_memory().unwrap();
    let file = FileRecord {
        path: "many.rs".to_string(),
        content_hash: "hash".to_string(),
        language: Language::Rust,
        size: 1000,
        modified_at: 0,
        indexed_at: 0,
        node_count: 0,
    };
    db.upsert_file(&file).unwrap();

    // Insert many similar nodes
    let mut extractor = Extractor::new();
    let code = (0..20)
        .map(|i| format!("fn process_item_{}() {{}}", i))
        .collect::<Vec<_>>()
        .join("\n");

    let result = extractor.extract_file("many.rs", &code);
    for mut node in result.nodes {
        node.id = 0;
        db.insert_node(&node).unwrap();
    }

    // Search with limit
    let results = db.search_nodes("process", None, 5).unwrap();
    assert_eq!(results.len(), 5);

    let results = db.search_nodes("process", None, 100).unwrap();
    assert_eq!(results.len(), 20);
}
