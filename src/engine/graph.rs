use crate::models::error::WorkflowError;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use crate::models::workflow::{Edge, Node, Workflow};

pub struct WorkflowGraph {
    pub dag: DiGraph<Node, Edge>,
    pub node_map: HashMap<String, NodeIndex>,
    pub hash: String,
}

impl WorkflowGraph {
    pub fn new() -> Self {
        Self {
            dag: DiGraph::new(),
            node_map: HashMap::new(),
            hash: String::new(),
        }
    }

    pub fn load(&mut self, json: &str) -> Result<(), WorkflowError> {
        let hash = compute_hash(json);
        if self.hash == hash {
            return Ok(());
        }

        // Clear existing graph and map because we have a new/updated workflow
        self.dag.clear();
        self.node_map.clear();

        let workflow: Workflow =
            serde_json::from_str(json).map_err(|e| WorkflowError::ParseError(e.to_string()))?;

        // 1. Load Nodes
        for node in workflow.nodes {
            let node_id = node.id.clone();
            let idx = self.dag.add_node(node);
            self.node_map.insert(node_id, idx);
        }

        // 2. Load Edges
        for edge in workflow.edges {
            let source_id = &edge.source;
            let target_id = &edge.target;

            let source_idx = self.node_map.get(source_id).copied();
            let target_idx = self.node_map.get(target_id).copied();

            if let (Some(s), Some(t)) = (source_idx, target_idx) {
                self.dag.add_edge(s, t, edge);
            } else {
                return Err(WorkflowError::InvalidGraph(format!(
                    "Edge references missing node: {} -> {}",
                    source_id, target_id
                )));
            }
        }

        let sink_count = self
            .dag
            .node_indices()
            .filter(|idx| {
                self.dag
                    .edges_directed(*idx, Direction::Outgoing)
                    .next()
                    .is_none()
            })
            .count();
        if self.dag.node_count() > 0 && sink_count != 1 {
            return Err(WorkflowError::InvalidGraph(format!(
                "Invalid end nodes count: {}",
                sink_count
            )));
        }

        self.hash = hash;

        Ok(())
    }
}

fn compute_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_workflow() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "id": "workflow-1",
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} },
                { "id": "node-2", "type": "end", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-2" }
            ]
        }"#;

        graph.load(json).unwrap();

        assert_eq!(graph.dag.node_count(), 2);
        assert_eq!(graph.dag.edge_count(), 1);
        assert!(graph.node_map.contains_key("node-1"));
        assert!(graph.node_map.contains_key("node-2"));

        let node1_idx = graph.node_map["node-1"];
        let node2_idx = graph.node_map["node-2"];

        // Verify edge
        assert!(graph.dag.find_edge(node1_idx, node2_idx).is_some());
    }

    #[test]
    fn test_load_workflow_idempotent() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "id": "workflow-1",
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} }
            ],
            "edges": []
        }"#;

        graph.load(json).unwrap();
        let initial_node_count = graph.dag.node_count();

        // Load same JSON again
        graph.load(json).unwrap();

        assert_eq!(graph.dag.node_count(), initial_node_count);
    }

    #[test]
    fn test_load_workflow_invalid_edge() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "id": "workflow-invalid",
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-2" }
            ]
        }"#;

        let result = graph.load(json);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidGraph(msg)) => {
                assert!(msg.contains("Edge references missing node"));
                assert!(msg.contains("node-1 -> node-2"));
            }
            _ => panic!("Expected InvalidGraph error"),
        }
    }

    #[test]
    fn test_load_workflow_multiple_end_nodes() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "id": "workflow-multi-end",
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} },
                { "id": "node-2", "type": "print", "data": {} },
                { "id": "node-3", "type": "end", "data": {} },
                { "id": "node-4", "type": "end", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-3" },
                { "source": "node-2", "target": "node-4" }
            ]
        }"#;

        let result = graph.load(json);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidGraph(msg)) => {
                assert!(msg.contains("Invalid end nodes count"));
            }
            _ => panic!("Expected InvalidGraph error"),
        }
    }
}
