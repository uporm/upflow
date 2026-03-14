use crate::models::error::WorkflowError;
use crate::models::workflow::{Edge, Node, Workflow};
use petgraph::Direction;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

// 工作流图结构体
// 包含一个有向无环图和一个节点ID到图索引的映射表
pub struct WorkflowGraph {
    pub dag: DiGraph<Node, Edge>, // 使用petgraph的有向无环图存储节点和边
    pub node_map: HashMap<String, NodeIndex>, // 节点ID到图索引的映射，便于快速查找
    pub topo_nodes: Vec<Node>,
}

impl WorkflowGraph {
    // 创建一个新的工作流图实例
    pub fn new() -> Self {
        Self {
            dag: DiGraph::new(),      // 初始化空的有向无环图
            node_map: HashMap::new(), // 初始化空的节点映射表
            topo_nodes: Vec::new(),
        }
    }

    // 根据给定的工作流构建内部图结构
    // 验证图的有效性（如只有一个结束节点）
    pub fn build(&mut self, workflow: Workflow) -> Result<(), WorkflowError> {
        // 清除现有的图和映射
        self.dag.clear();
        self.node_map.clear();

        // 1. 加载节点
        for node in workflow.nodes {
            let node_id = node.id.clone();
            let idx = self.dag.add_node(node);
            self.node_map.insert(node_id, idx); // 将节点ID与其在DAG中的索引建立映射关系
        }

        // 2. 加载边
        for edge in workflow.edges {
            let source_id = &edge.source;
            let target_id = &edge.target;

            let source_idx = self.node_map.get(source_id).copied();
            let target_idx = self.node_map.get(target_id).copied();

            // 检查源节点和目标节点是否都存在
            if let (Some(s), Some(t)) = (source_idx, target_idx) {
                self.dag.add_edge(s, t, edge);
            } else {
                // 如果引用的节点不存在，返回错误
                return Err(WorkflowError::InvalidGraph(format!(
                    "Edge references missing node: {} -> {}",
                    source_id, target_id
                )));
            }
        }

        if is_cyclic_directed(&self.dag) {
            return Err(WorkflowError::InvalidGraph(
                "Workflow contains cycle detected".to_string(),
            ));
        }

        // 计算起始节点数量（没有入边的节点）
        let start_node_count = self.count_nodes_without_edges(Direction::Incoming);

        if self.dag.node_count() > 0 && start_node_count == 0 {
            return Err(WorkflowError::InvalidGraph(
                "Workflow contains no start node".to_string(),
            ));
        }

        // 计算结束节点数量（没有出边的节点）
        let end_node_count = self.count_nodes_without_edges(Direction::Outgoing);

        // 如果图不为空但终点节点数量不是1个，则返回错误
        // 工作流必须有且仅有一个终点
        if self.dag.node_count() > 0 && end_node_count != 1 {
            return Err(WorkflowError::InvalidGraph(format!(
                "Workflow contains invalid end nodes count: {}",
                end_node_count
            )));
        }

        let order = toposort(&self.dag, None).map_err(|_| {
            WorkflowError::InvalidGraph("Workflow contains cycle detected".to_string())
        })?;
        self.topo_nodes = order.into_iter().map(|idx| self.dag[idx].clone()).collect();
        Ok(())
    }

    pub fn topological_nodes(&self) -> Result<Vec<Node>, WorkflowError> {
        Ok(self.topo_nodes.clone())
    }

    // 辅助方法：计算没有特定方向边的节点数量
    fn count_nodes_without_edges(&self, direction: Direction) -> usize {
        self.dag
            .node_indices()
            .filter(|idx| self.dag.edges_directed(*idx, direction).next().is_none())
            .count()
    }
}

// 单元测试模块
#[cfg(test)]
mod tests {
    use super::*;

    // 测试加载基本工作流的功能
    #[test]
    fn test_load_workflow() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} },
                { "id": "node-2", "type": "end", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-2" }
            ]
        }"#;
        let workflow: Workflow = serde_json::from_str(json).unwrap();

        graph.build(workflow).unwrap();

        // 验证图的节点和边数量
        assert_eq!(graph.dag.node_count(), 2);
        assert_eq!(graph.dag.edge_count(), 1);
        assert!(graph.node_map.contains_key("node-1"));
        assert!(graph.node_map.contains_key("node-2"));

        let node1_idx = graph.node_map["node-1"];
        let node2_idx = graph.node_map["node-2"];

        // 验证边是否存在
        assert!(graph.dag.find_edge(node1_idx, node2_idx).is_some());
    }

    // 测试工作流构建的幂等性（重复构建相同工作流）
    #[test]
    fn test_load_workflow_idempotent() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} }
            ],
            "edges": []
        }"#;
        let workflow: Workflow = serde_json::from_str(json).unwrap();

        graph.build(workflow.clone()).unwrap();
        let initial_node_count = graph.dag.node_count();

        // 再次加载相同的JSON
        graph.build(workflow).unwrap();

        assert_eq!(graph.dag.node_count(), initial_node_count);
    }

    // 测试加载包含无效边的工作流（引用不存在的节点）
    #[test]
    fn test_load_workflow_invalid_edge() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "nodes": [
                { "id": "node-1", "type": "start", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-2" }
            ]
        }"#;
        let workflow: Workflow = serde_json::from_str(json).unwrap();

        let result = graph.build(workflow);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidGraph(msg)) => {
                assert!(msg.contains("Edge references missing node"));
                assert!(msg.contains("node-1 -> node-2"));
            }
            _ => panic!("Expected InvalidGraph error"),
        }
    }

    // 测试加载包含多个结束节点的工作流（应该失败）
    // 验证工作流只能有一个终点的约束
    #[test]
    fn test_load_workflow_multiple_end_nodes() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
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
        let workflow: Workflow = serde_json::from_str(json).unwrap();

        let result = graph.build(workflow);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidGraph(msg)) => {
                assert!(msg.contains("invalid end nodes count"));
            }
            _ => panic!("Expected InvalidGraph error"),
        }
    }

    #[test]
    fn test_load_workflow_cycle_detected() {
        let mut graph = WorkflowGraph::new();
        let json = r#"{
            "id": "workflow-no-start",
            "nodes": [
                { "id": "node-1", "type": "task", "data": {} },
                { "id": "node-2", "type": "task", "data": {} }
            ],
            "edges": [
                { "source": "node-1", "target": "node-2" },
                { "source": "node-2", "target": "node-1" }
            ]
        }"#;
        let workflow: Workflow = serde_json::from_str(json).unwrap();

        let result = graph.build(workflow);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidGraph(msg)) => {
                assert!(msg.contains("cycle detected"));
            }
            _ => panic!("Expected InvalidGraph error"),
        }
    }
}
