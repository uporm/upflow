use crate::core::constants::GROUP_FLOW_ID_KEY;
use crate::core::enums::NodeType;
use crate::models::workflow::{Edge, Node, Workflow};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;

pub fn split_workflow_for_groups(
    workflow_id: &str,
    workflow: Workflow,
) -> (Workflow, Vec<(String, Workflow)>) {
    let Workflow { nodes, edges, .. } = workflow;
    let mut group_ids = HashSet::new();
    for node in &nodes {
        if node.node_type == NodeType::Group.to_str() {
            group_ids.insert(node.id.clone());
        }
    }
    if group_ids.is_empty() {
        return (
            Workflow {
                nodes,
                edges,
            },
            Vec::new(),
        );
    }

    let mut group_children: HashMap<String, Vec<Node>> = HashMap::new();
    let mut child_to_group: HashMap<String, String> = HashMap::new();
    let mut main_nodes: Vec<Node> = Vec::new();

    for node in nodes {
        if let Some(parent_id) = node.parent_id.as_ref() {
            if group_ids.contains(parent_id) {
                child_to_group.insert(node.id.clone(), parent_id.clone());
                group_children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(node);
                continue;
            }
        }
        let mut node = node;
        attach_subflow_id(&mut node, workflow_id);
        main_nodes.push(node);
    }

    let main_node_ids: HashSet<String> = main_nodes.iter().map(|n| n.id.clone()).collect();
    let mut main_edges: Vec<Edge> = Vec::new();
    let mut group_edges: HashMap<String, Vec<Edge>> = HashMap::new();

    for edge in edges {
        let source_group = child_to_group.get(&edge.source);
        let target_group = child_to_group.get(&edge.target);
        if let (Some(g1), Some(g2)) = (source_group, target_group) {
            if g1 == g2 {
                group_edges.entry(g1.clone()).or_default().push(edge);
                continue;
            }
        }
        if main_node_ids.contains(&edge.source) && main_node_ids.contains(&edge.target) {
            main_edges.push(edge);
        }
    }

    let mut subflows = Vec::new();
    for group_id in group_ids {
        let subflow_id = format!("{}_{}", workflow_id, group_id);
        let mut nodes = group_children.remove(&group_id).unwrap_or_default();
        for node in &mut nodes {
            node.parent_id = None;
            attach_subflow_id(node, workflow_id);
        }
        let edges = group_edges.remove(&group_id).unwrap_or_default();
        let subflow = Workflow {
            nodes,
            edges,
        };
        subflows.push((subflow_id, subflow));
    }

    (
        Workflow {
            nodes: main_nodes,
            edges: main_edges,
        },
        subflows,
    )
}

fn attach_subflow_id(node: &mut Node, workflow_id: &str) {
    if node.node_type != "group" {
        return;
    }
    let subflow_id = format!("{}_{}", workflow_id, node.id);
    let data = Arc::make_mut(&mut node.data);
    let mut map = match mem::take(data) {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    map.insert(GROUP_FLOW_ID_KEY.to_string(), Value::String(subflow_id));
    *data = Value::Object(map);
}
