//! Core type definitions for codemap
//!
//! Defines the fundamental types for representing code structure:
//! - Nodes: code symbols (functions, classes, methods, etc.)
//! - Edges: relationships between nodes (calls, contains, imports, etc.)
//! - Languages: supported programming languages

use serde::{Deserialize, Serialize};

/// Represents the kind of code symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Module,
    Class,
    Struct,
    Interface,
    Trait,
    Protocol,
    Function,
    Method,
    Property,
    Field,
    Variable,
    Constant,
    Enum,
    EnumMember,
    TypeAlias,
    Namespace,
    Parameter,
    Import,
    Export,
    Route,
    Component,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Module => "module",
            NodeKind::Class => "class",
            NodeKind::Struct => "struct",
            NodeKind::Interface => "interface",
            NodeKind::Trait => "trait",
            NodeKind::Protocol => "protocol",
            NodeKind::Function => "function",
            NodeKind::Method => "method",
            NodeKind::Property => "property",
            NodeKind::Field => "field",
            NodeKind::Variable => "variable",
            NodeKind::Constant => "constant",
            NodeKind::Enum => "enum",
            NodeKind::EnumMember => "enum_member",
            NodeKind::TypeAlias => "type_alias",
            NodeKind::Namespace => "namespace",
            NodeKind::Parameter => "parameter",
            NodeKind::Import => "import",
            NodeKind::Export => "export",
            NodeKind::Route => "route",
            NodeKind::Component => "component",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "file" => Some(NodeKind::File),
            "module" => Some(NodeKind::Module),
            "class" => Some(NodeKind::Class),
            "struct" => Some(NodeKind::Struct),
            "interface" => Some(NodeKind::Interface),
            "trait" => Some(NodeKind::Trait),
            "protocol" => Some(NodeKind::Protocol),
            "function" => Some(NodeKind::Function),
            "method" => Some(NodeKind::Method),
            "property" => Some(NodeKind::Property),
            "field" => Some(NodeKind::Field),
            "variable" => Some(NodeKind::Variable),
            "constant" => Some(NodeKind::Constant),
            "enum" => Some(NodeKind::Enum),
            "enum_member" => Some(NodeKind::EnumMember),
            "type_alias" => Some(NodeKind::TypeAlias),
            "namespace" => Some(NodeKind::Namespace),
            "parameter" => Some(NodeKind::Parameter),
            "import" => Some(NodeKind::Import),
            "export" => Some(NodeKind::Export),
            "route" => Some(NodeKind::Route),
            "component" => Some(NodeKind::Component),
            _ => None,
        }
    }
}

/// Represents the kind of relationship between nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Parent contains child (e.g., class contains method)
    Contains,
    /// Source calls target function/method
    Calls,
    /// Source imports target module/symbol
    Imports,
    /// Source exports target symbol
    Exports,
    /// Source extends target (inheritance)
    Extends,
    /// Source implements target interface/trait
    Implements,
    /// Source references target symbol
    References,
    /// Source has type target
    TypeOf,
    /// Source returns target type
    Returns,
    /// Source instantiates target class
    Instantiates,
    /// Source overrides target method
    Overrides,
    /// Source is decorated by target
    Decorates,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Calls => "calls",
            EdgeKind::Imports => "imports",
            EdgeKind::Exports => "exports",
            EdgeKind::Extends => "extends",
            EdgeKind::Implements => "implements",
            EdgeKind::References => "references",
            EdgeKind::TypeOf => "type_of",
            EdgeKind::Returns => "returns",
            EdgeKind::Instantiates => "instantiates",
            EdgeKind::Overrides => "overrides",
            EdgeKind::Decorates => "decorates",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(EdgeKind::Contains),
            "calls" => Some(EdgeKind::Calls),
            "imports" => Some(EdgeKind::Imports),
            "exports" => Some(EdgeKind::Exports),
            "extends" => Some(EdgeKind::Extends),
            "implements" => Some(EdgeKind::Implements),
            "references" => Some(EdgeKind::References),
            "type_of" => Some(EdgeKind::TypeOf),
            "returns" => Some(EdgeKind::Returns),
            "instantiates" => Some(EdgeKind::Instantiates),
            "overrides" => Some(EdgeKind::Overrides),
            "decorates" => Some(EdgeKind::Decorates),
            _ => None,
        }
    }
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Tsx,
    Jsx,
    Python,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Php,
    Ruby,
    Swift,
    Kotlin,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "ts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "jsx" => Language::Jsx,
            "py" | "pyi" => Language::Python,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Language::Cpp,
            "cs" => Language::CSharp,
            "php" => Language::Php,
            "rb" => Language::Ruby,
            "swift" => Language::Swift,
            "kt" | "kts" => Language::Kotlin,
            _ => Language::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Tsx => "tsx",
            Language::Jsx => "jsx",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Php => "php",
            Language::Ruby => "ruby",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Unknown => "unknown",
        }
    }
}

/// A location in source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Visibility modifier for symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
    Unknown,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
            Visibility::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "public" | "pub" => Visibility::Public,
            "private" | "priv" => Visibility::Private,
            "protected" => Visibility::Protected,
            "internal" => Visibility::Internal,
            _ => Visibility::Unknown,
        }
    }
}

/// A code symbol (function, class, method, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: i64,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: Option<String>,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub visibility: Visibility,
    pub docstring: Option<String>,
    pub is_async: bool,
    pub is_static: bool,
    pub is_exported: bool,
    pub language: Language,
}

/// A relationship between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub kind: EdgeKind,
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Metadata about an indexed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub content_hash: String,
    pub language: Language,
    pub size: u64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub node_count: u32,
}

/// An unresolved reference found during extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReference {
    pub source_node_id: i64,
    pub reference_name: String,
    pub kind: EdgeKind,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

/// Result of extracting symbols from a file
#[derive(Debug, Clone, Default)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_refs: Vec<UnresolvedReference>,
    pub errors: Vec<ExtractionError>,
}

/// Error during extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionError {
    pub message: String,
    pub file_path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Search result from the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
    pub code_snippet: Option<String>,
}

/// Options for graph traversal
#[derive(Debug, Clone)]
pub struct TraversalOptions {
    pub max_depth: u32,
    pub edge_kinds: Option<Vec<EdgeKind>>,
    pub node_kinds: Option<Vec<NodeKind>>,
    pub limit: u32,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            edge_kinds: None,
            node_kinds: None,
            limit: 50,
        }
    }
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_files: u64,
    pub total_nodes: u64,
    pub total_edges: u64,
    pub db_size_bytes: u64,
    pub languages: Vec<(Language, u64)>,
    pub node_kinds: Vec<(NodeKind, u64)>,
}

/// Context built for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub entry_points: Vec<Node>,
    pub related_nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub code_blocks: Vec<CodeBlock>,
}

/// A code block with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub node: Node,
    pub code: String,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}
