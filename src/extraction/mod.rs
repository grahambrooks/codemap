//! Code extraction module
//!
//! Uses tree-sitter to parse source code and extract:
//! - Symbols (functions, classes, methods, etc.)
//! - Relationships (calls, contains, imports, etc.)

mod languages;

use std::path::Path;
use tree_sitter::Parser;

use crate::types::{
    Edge, EdgeKind, ExtractionError, ExtractionResult, Language, Node, NodeKind,
    UnresolvedReference, Visibility,
};

use languages::LanguageConfig;

/// Extracts code symbols from source files using tree-sitter
pub struct Extractor {
    parser: Parser,
}

impl Extractor {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
        }
    }

    /// Extract symbols from a source file
    pub fn extract_file<P: AsRef<Path>>(&mut self, path: P, content: &str) -> ExtractionResult {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let language = Language::from_extension(ext);

        if language == Language::Unknown {
            return ExtractionResult {
                errors: vec![ExtractionError {
                    message: format!("Unsupported file extension: {}", ext),
                    file_path: path.display().to_string(),
                    line: None,
                    column: None,
                }],
                ..Default::default()
            };
        }

        let Some(ts_lang) = languages::get_language(language) else {
            return ExtractionResult {
                errors: vec![ExtractionError {
                    message: format!("No tree-sitter grammar for {:?}", language),
                    file_path: path.display().to_string(),
                    line: None,
                    column: None,
                }],
                ..Default::default()
            };
        };

        if self.parser.set_language(&ts_lang).is_err() {
            return ExtractionResult {
                errors: vec![ExtractionError {
                    message: "Failed to set parser language".to_string(),
                    file_path: path.display().to_string(),
                    line: None,
                    column: None,
                }],
                ..Default::default()
            };
        }

        let Some(tree) = self.parser.parse(content, None) else {
            return ExtractionResult {
                errors: vec![ExtractionError {
                    message: "Failed to parse file".to_string(),
                    file_path: path.display().to_string(),
                    line: None,
                    column: None,
                }],
                ..Default::default()
            };
        };

        let config = languages::get_config(language);
        let file_path = path.display().to_string();

        let mut ctx = ExtractionContext {
            result: ExtractionResult::default(),
            file_path: file_path.clone(),
            content,
            language,
            config,
            node_stack: Vec::new(),
            next_id: 1,
        };

        // Create file node
        let file_node = Node {
            id: ctx.next_id,
            kind: NodeKind::File,
            name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            qualified_name: Some(file_path.clone()),
            file_path: file_path.clone(),
            start_line: 0,
            end_line: content.lines().count() as u32,
            start_column: 0,
            end_column: 0,
            signature: None,
            visibility: Visibility::Public,
            docstring: None,
            is_async: false,
            is_static: false,
            is_exported: true,
            language,
        };
        ctx.next_id += 1;
        ctx.result.nodes.push(file_node);
        ctx.node_stack.push(1); // file node ID

        // Traverse the tree
        ctx.traverse_node(tree.root_node());

        ctx.result
    }
}

struct ExtractionContext<'a> {
    result: ExtractionResult,
    file_path: String,
    content: &'a str,
    language: Language,
    config: &'static LanguageConfig,
    node_stack: Vec<i64>, // Stack of parent node IDs
    next_id: i64,
}

impl<'a> ExtractionContext<'a> {
    fn traverse_node(&mut self, node: tree_sitter::Node) {
        let node_type = node.kind();

        // Check if this is a symbol we care about
        if let Some(kind) = self.config.node_type_to_kind(node_type) {
            self.extract_symbol(node, kind);
        } else {
            // Continue traversing children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.traverse_node(child);
            }
        }
    }

    fn extract_symbol(&mut self, node: tree_sitter::Node, kind: NodeKind) {
        let name = self.extract_name(&node, kind);
        if name.is_empty() {
            // Skip anonymous nodes
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.traverse_node(child);
            }
            return;
        }

        let start = node.start_position();
        let end = node.end_position();

        let symbol = Node {
            id: self.next_id,
            kind,
            name: name.clone(),
            qualified_name: self.build_qualified_name(&name),
            file_path: self.file_path.clone(),
            start_line: start.row as u32 + 1,
            end_line: end.row as u32 + 1,
            start_column: start.column as u32,
            end_column: end.column as u32,
            signature: self.extract_signature(&node, kind),
            visibility: self.extract_visibility(&node),
            docstring: self.extract_docstring(&node),
            is_async: self.check_async(&node),
            is_static: self.check_static(&node),
            is_exported: self.check_exported(&node),
            language: self.language,
        };

        let symbol_id = self.next_id;
        self.next_id += 1;
        self.result.nodes.push(symbol);

        // Create contains edge from parent
        if let Some(&parent_id) = self.node_stack.last() {
            let edge = Edge {
                id: 0,
                source_id: parent_id,
                target_id: symbol_id,
                kind: EdgeKind::Contains,
                file_path: Some(self.file_path.clone()),
                line: Some(start.row as u32 + 1),
                column: Some(start.column as u32),
            };
            self.result.edges.push(edge);
        }

        // Push this symbol onto the stack and traverse children
        self.node_stack.push(symbol_id);

        // Extract function calls and other references from body
        self.extract_references(&node, symbol_id);

        // Traverse children for nested definitions
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_node(child);
        }

        self.node_stack.pop();
    }

    fn extract_name(&self, node: &tree_sitter::Node, _kind: NodeKind) -> String {
        // Try to find name child
        for field_name in &["name", "declarator", "identifier"] {
            if let Some(name_node) = node.child_by_field_name(field_name) {
                let name = self.get_node_text(&name_node);
                if !name.is_empty() {
                    // Handle pointer declarators in C/C++
                    if name_node.kind() == "pointer_declarator" || name_node.kind() == "function_declarator" {
                        if let Some(id) = name_node.child_by_field_name("declarator") {
                            return self.get_node_text(&id);
                        }
                    }
                    return name;
                }
            }
        }

        // For some languages, look at specific child positions
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                return self.get_node_text(&child);
            }
        }

        String::new()
    }

    fn extract_signature(&self, node: &tree_sitter::Node, kind: NodeKind) -> Option<String> {
        match kind {
            NodeKind::Function | NodeKind::Method => {
                // Get the first line or until opening brace
                let text = self.get_node_text(node);
                let sig = text.lines().next().unwrap_or("");
                // Truncate at opening brace or newline
                let sig = sig.split('{').next().unwrap_or(sig).trim();
                if sig.len() > 200 {
                    Some(format!("{}...", &sig[..200]))
                } else {
                    Some(sig.to_string())
                }
            }
            NodeKind::Class | NodeKind::Struct | NodeKind::Interface | NodeKind::Trait => {
                let text = self.get_node_text(node);
                let sig = text.lines().next().unwrap_or("");
                let sig = sig.split('{').next().unwrap_or(sig).trim();
                Some(sig.to_string())
            }
            _ => None,
        }
    }

    fn extract_visibility(&self, node: &tree_sitter::Node) -> Visibility {
        let text = self.get_node_text(node);
        let first_line = text.lines().next().unwrap_or("");

        if first_line.starts_with("pub ") || first_line.contains(" pub ") {
            return Visibility::Public;
        }
        if first_line.starts_with("public ") || first_line.contains(" public ") {
            return Visibility::Public;
        }
        if first_line.starts_with("private ") || first_line.contains(" private ") {
            return Visibility::Private;
        }
        if first_line.starts_with("protected ") || first_line.contains(" protected ") {
            return Visibility::Protected;
        }

        // Check for visibility modifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "visibility_modifier" || kind == "access_specifier" {
                let vis_text = self.get_node_text(&child);
                return Visibility::from_str(&vis_text);
            }
        }

        Visibility::Unknown
    }

    fn extract_docstring(&self, node: &tree_sitter::Node) -> Option<String> {
        // Look for comment before this node
        if let Some(prev) = node.prev_sibling() {
            let kind = prev.kind();
            if kind.contains("comment") || kind == "doc_comment" || kind == "block_comment" {
                let text = self.get_node_text(&prev);
                return Some(self.clean_docstring(&text));
            }
        }
        None
    }

    fn clean_docstring(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("///")
                    .trim_start_matches("//!")
                    .trim_start_matches("//")
                    .trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_start_matches('*')
                    .trim_end_matches("*/")
                    .trim()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn check_async(&self, node: &tree_sitter::Node) -> bool {
        let text = self.get_node_text(node);
        text.starts_with("async ") || text.contains(" async ")
    }

    fn check_static(&self, node: &tree_sitter::Node) -> bool {
        let text = self.get_node_text(node);
        text.starts_with("static ") || text.contains(" static ")
    }

    fn check_exported(&self, node: &tree_sitter::Node) -> bool {
        let text = self.get_node_text(node);
        // Rust pub
        if text.starts_with("pub ") {
            return true;
        }
        // JS/TS export
        if text.starts_with("export ") {
            return true;
        }
        // Check for export default
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return true;
            }
        }
        false
    }

    fn build_qualified_name(&self, name: &str) -> Option<String> {
        let mut parts = Vec::new();
        for &parent_id in &self.node_stack {
            if let Some(parent) = self.result.nodes.iter().find(|n| n.id == parent_id) {
                if parent.kind != NodeKind::File {
                    parts.push(parent.name.clone());
                }
            }
        }
        parts.push(name.to_string());
        Some(parts.join("::"))
    }

    fn extract_references(&mut self, node: &tree_sitter::Node, source_id: i64) {
        // Find call expressions within this node
        self.find_calls(node, source_id);
    }

    fn find_calls(&mut self, node: &tree_sitter::Node, source_id: i64) {
        let kind = node.kind();

        if self.config.is_call_node(kind) {
            if let Some(func_name) = self.extract_call_name(node) {
                let start = node.start_position();
                let uref = UnresolvedReference {
                    source_node_id: source_id,
                    reference_name: func_name,
                    kind: EdgeKind::Calls,
                    file_path: self.file_path.clone(),
                    line: start.row as u32 + 1,
                    column: start.column as u32,
                };
                self.result.unresolved_refs.push(uref);
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_calls(&child, source_id);
        }
    }

    fn extract_call_name(&self, node: &tree_sitter::Node) -> Option<String> {
        // Look for function name in call expression
        if let Some(func) = node.child_by_field_name("function") {
            let text = self.get_node_text(&func);
            // Handle method calls: obj.method() -> method
            if let Some(dot_pos) = text.rfind('.') {
                return Some(text[dot_pos + 1..].to_string());
            }
            // Handle path calls: foo::bar() -> bar
            if let Some(colon_pos) = text.rfind("::") {
                return Some(text[colon_pos + 2..].to_string());
            }
            return Some(text);
        }

        // Try first child as fallback
        if let Some(first) = node.child(0) {
            if first.kind() == "identifier" || first.kind() == "field_expression" {
                let text = self.get_node_text(&first);
                if let Some(dot_pos) = text.rfind('.') {
                    return Some(text[dot_pos + 1..].to_string());
                }
                return Some(text);
            }
        }

        None
    }

    fn get_node_text(&self, node: &tree_sitter::Node) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        if start < self.content.len() && end <= self.content.len() {
            self.content[start..end].to_string()
        } else {
            String::new()
        }
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self::new()
    }
}
