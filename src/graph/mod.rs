use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::model::{Edge, Node, Workflow, WorkflowError};

pub struct GraphStore {
    pub graph: DiGraph<Node, Edge>,
    pub node_map: HashMap<String, NodeIndex>,
}

impl GraphStore {
    pub fn build(def: &Workflow) -> Result<Self, WorkflowError> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();
        for node in &def.nodes {
            let idx = graph.add_node(node.clone());
            node_map.insert(node.id.clone(), idx);
        }
        for edge in &def.edges {
            let source = node_map
                .get(&edge.source)
                .ok_or_else(|| WorkflowError::InvalidGraph(edge.source.clone()))?;
            let target = node_map
                .get(&edge.target)
                .ok_or_else(|| WorkflowError::InvalidGraph(edge.target.clone()))?;
            graph.add_edge(*source, *target, edge.clone());
        }
        toposort(&graph, None)
            .map_err(|e| WorkflowError::InvalidGraph(format!("cycle at {:?}", e.node_id())))?;
        Ok(Self { graph, node_map })
    }
}
