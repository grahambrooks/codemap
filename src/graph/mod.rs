//! Graph traversal and query operations
//!
//! Provides algorithms for:
//! - Finding callers/callees
//! - Impact analysis
//! - Subgraph extraction

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;

use crate::db::Database;
use crate::types::{Edge, Node, TraversalOptions};

/// Graph operations on the code database
pub struct Graph<'a> {
    db: &'a Database,
}

impl<'a> Graph<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Find all callers of a symbol (functions that call this function)
    pub fn find_callers(&self, symbol_name: &str, limit: u32) -> Result<Vec<Node>> {
        // First, find the node by name
        let target = match self.db.find_node_by_name(symbol_name)? {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        self.db.get_callers(target.id, limit)
    }

    /// Find all callees of a symbol (functions that this function calls)
    pub fn find_callees(&self, symbol_name: &str, limit: u32) -> Result<Vec<Node>> {
        let source = match self.db.find_node_by_name(symbol_name)? {
            Some(node) => node,
            None => return Ok(Vec::new()),
        };

        self.db.get_callees(source.id, limit)
    }

    /// Analyze the impact of changing a symbol
    /// Returns all symbols that could be affected by the change
    pub fn analyze_impact(&self, symbol_name: &str, depth: u32) -> Result<ImpactAnalysis> {
        let root = match self.db.find_node_by_name(symbol_name)? {
            Some(node) => node,
            None => {
                return Ok(ImpactAnalysis {
                    root: None,
                    direct_callers: Vec::new(),
                    indirect_callers: Vec::new(),
                    total_impact: 0,
                })
            }
        };

        let mut visited: HashSet<i64> = HashSet::new();
        let mut direct_callers = Vec::new();
        let mut indirect_callers = Vec::new();

        visited.insert(root.id);

        // BFS to find all callers up to depth
        let mut queue: VecDeque<(i64, u32)> = VecDeque::new();
        queue.push_back((root.id, 0));

        while let Some((node_id, current_depth)) = queue.pop_front() {
            if current_depth >= depth {
                continue;
            }

            // Get callers of this node
            let callers = self.db.get_callers(node_id, 100)?;

            for caller in callers {
                if visited.contains(&caller.id) {
                    continue;
                }
                visited.insert(caller.id);

                if current_depth == 0 {
                    direct_callers.push(caller.clone());
                } else {
                    indirect_callers.push(caller.clone());
                }

                queue.push_back((caller.id, current_depth + 1));
            }
        }

        let total_impact = direct_callers.len() + indirect_callers.len();

        Ok(ImpactAnalysis {
            root: Some(root),
            direct_callers,
            indirect_callers,
            total_impact,
        })
    }

    /// Extract a subgraph around a set of nodes
    pub fn extract_subgraph(
        &self,
        node_ids: &[i64],
        options: &TraversalOptions,
    ) -> Result<Subgraph> {
        let mut nodes: HashMap<i64, Node> = HashMap::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut visited: HashSet<i64> = HashSet::new();

        // Start with the seed nodes
        for &id in node_ids {
            if let Some(node) = self.db.get_node(id)? {
                nodes.insert(id, node);
                visited.insert(id);
            }
        }

        // BFS expansion
        let mut queue: VecDeque<(i64, u32)> = node_ids.iter().map(|&id| (id, 0)).collect();

        while let Some((node_id, depth)) = queue.pop_front() {
            if depth >= options.max_depth {
                continue;
            }

            // Get outgoing edges
            let out_edges = self.db.get_outgoing_edges(node_id)?;
            for edge in out_edges {
                // Filter by edge kind if specified
                if let Some(ref kinds) = options.edge_kinds {
                    if !kinds.contains(&edge.kind) {
                        continue;
                    }
                }

                edges.push(edge.clone());

                if !visited.contains(&edge.target_id) {
                    if let Some(target) = self.db.get_node(edge.target_id)? {
                        // Filter by node kind if specified
                        if let Some(ref kinds) = options.node_kinds {
                            if !kinds.contains(&target.kind) {
                                continue;
                            }
                        }

                        visited.insert(edge.target_id);
                        nodes.insert(edge.target_id, target);
                        queue.push_back((edge.target_id, depth + 1));

                        if nodes.len() >= options.limit as usize {
                            break;
                        }
                    }
                }
            }

            // Get incoming edges
            let in_edges = self.db.get_incoming_edges(node_id)?;
            for edge in in_edges {
                if let Some(ref kinds) = options.edge_kinds {
                    if !kinds.contains(&edge.kind) {
                        continue;
                    }
                }

                edges.push(edge.clone());

                if !visited.contains(&edge.source_id) {
                    if let Some(source) = self.db.get_node(edge.source_id)? {
                        if let Some(ref kinds) = options.node_kinds {
                            if !kinds.contains(&source.kind) {
                                continue;
                            }
                        }

                        visited.insert(edge.source_id);
                        nodes.insert(edge.source_id, source);
                        queue.push_back((edge.source_id, depth + 1));

                        if nodes.len() >= options.limit as usize {
                            break;
                        }
                    }
                }
            }

            if nodes.len() >= options.limit as usize {
                break;
            }
        }

        Ok(Subgraph {
            nodes: nodes.into_values().collect(),
            edges,
        })
    }

    /// Find related symbols given a set of entry points
    pub fn find_related(
        &self,
        entry_points: &[Node],
        max_nodes: u32,
    ) -> Result<Vec<Node>> {
        let mut related: HashMap<i64, (Node, f64)> = HashMap::new();
        let mut visited: HashSet<i64> = HashSet::new();

        for entry in entry_points {
            visited.insert(entry.id);
        }

        // For each entry point, find its neighbors
        for entry in entry_points {
            // Callees (what this function calls)
            let callees = self.db.get_callees(entry.id, 10)?;
            for (idx, callee) in callees.into_iter().enumerate() {
                if !visited.contains(&callee.id) {
                    let score = 1.0 / (idx as f64 + 1.0);
                    related
                        .entry(callee.id)
                        .and_modify(|(_, s)| *s += score)
                        .or_insert((callee, score));
                }
            }

            // Callers (what calls this function)
            let callers = self.db.get_callers(entry.id, 10)?;
            for (idx, caller) in callers.into_iter().enumerate() {
                if !visited.contains(&caller.id) {
                    let score = 0.8 / (idx as f64 + 1.0);
                    related
                        .entry(caller.id)
                        .and_modify(|(_, s)| *s += score)
                        .or_insert((caller, score));
                }
            }
        }

        // Sort by score and return top N
        let mut sorted: Vec<_> = related.into_values().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(max_nodes as usize);

        Ok(sorted.into_iter().map(|(node, _)| node).collect())
    }
}

/// Result of impact analysis
#[derive(Debug, Clone)]
pub struct ImpactAnalysis {
    pub root: Option<Node>,
    pub direct_callers: Vec<Node>,
    pub indirect_callers: Vec<Node>,
    pub total_impact: usize,
}

/// A subgraph extracted from the code graph
#[derive(Debug, Clone)]
pub struct Subgraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
