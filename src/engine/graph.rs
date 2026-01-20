use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use serde_json;
use sha2::{Digest, Sha256};
use crate::model::error::WorkflowError;
use crate::model::workflow::{Edge, Node, Workflow};

struct WorkflowEntry {
    hash: String,
    workflow: Arc<Workflow>,
    graph: Arc<GraphStore>,
}

pub struct WorkflowStore {
    entries: RwLock<HashMap<String, WorkflowEntry>>,
}

impl WorkflowStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn load(
        &self,
        workflow_id: &str,
        json: &str,
        validate: impl Fn(&Workflow) -> Result<(), WorkflowError>,
    ) -> Result<(), WorkflowError> {
        let hash = compute_hash(json);
        {
            let entries = self.entries.read().unwrap();
            if let Some(entry) = entries.get(workflow_id) {
                if entry.hash == hash {
                    return Ok(());
                }
            }
        }
        let def: Workflow =
            serde_json::from_str(json).map_err(|e| WorkflowError::ParseError(e.to_string()))?;
        validate(&def)?;
        let graph = Arc::new(GraphStore::build(&def)?);
        let mut entries = self.entries.write().unwrap();
        entries.insert(
            workflow_id.to_string(),
            WorkflowEntry {
                hash,
                workflow: Arc::new(def),
                graph,
            },
        );
        Ok(())
    }

    pub fn get(
        &self,
        workflow_id: &str,
    ) -> Result<(Arc<Workflow>, Arc<GraphStore>), WorkflowError> {
        let entries = self.entries.read().unwrap();
        entries
            .get(workflow_id)
            .map(|entry| (entry.workflow.clone(), entry.graph.clone()))
            .ok_or_else(|| WorkflowError::WorkflowNotFound(workflow_id.to_string()))
    }
}

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

fn compute_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}
