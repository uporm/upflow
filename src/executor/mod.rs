use crate::context::WorkflowContext;
use crate::engine::WorkflowEngine;
use crate::error::WorkflowError;
use crate::model::{Edge, Node, WorkflowDefinition};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct WorkflowRunner {
    definition: Arc<WorkflowDefinition>,
}

#[derive(Debug)]
enum EngineMessage {
    /// NodeID, IsActive, Sender
    RunNode(String, bool, mpsc::Sender<EngineMessage>),
}

#[derive(Debug)]
struct NodeState {
    in_degree: i32,
    is_active: bool,
}

impl WorkflowRunner {
    pub fn new(def: Arc<WorkflowDefinition>) -> Self {
        Self { definition: def }
    }

    pub async fn run(&self, ctx: Arc<WorkflowContext>) -> Result<(), WorkflowError> {
        // 1. 预处理：构建查找表和邻接表
        // 优化：使用 HashMap 替代 Vec 遍历查找 (O(1) vs O(N))
        let node_map: Arc<HashMap<String, Node>> = Arc::new(
            self.definition
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.clone()))
                .collect(),
        );

        let mut adjacency: HashMap<String, Vec<Edge>> = HashMap::new();
        let mut node_states: HashMap<String, NodeState> = HashMap::new();

        // 初始化状态
        for node in &self.definition.nodes {
            node_states.insert(
                node.id.clone(),
                NodeState {
                    in_degree: 0,
                    is_active: false,
                },
            );
        }

        // 构建 DAG 关系
        for edge in &self.definition.edges {
            if let Some(state) = node_states.get_mut(&edge.target) {
                state.in_degree += 1;
            }
            adjacency
                .entry(edge.source.clone())
                .or_default()
                .push(edge.clone());
        }

        // 优化：合并 in_degree 和 active_nodes 为单一状态锁，减少锁竞争
        let state_lock = Arc::new(Mutex::new(node_states));
        let adjacency = Arc::new(adjacency);
        let (tx, mut rx) = mpsc::channel(100);

        // 2. 启动入口节点 (入度为 0)
        {
            let mut states = state_lock.lock().await;
            for (id, state) in states.iter_mut() {
                if state.in_degree == 0 {
                    // 入口节点默认激活
                    state.is_active = true;
                    tx.send(EngineMessage::RunNode(id.clone(), true, tx.clone()))
                        .await
                        .map_err(|e| WorkflowError::RuntimeError(e.to_string()))?;
                }
            }
        }

        // 显式 drop tx，这样当所有子任务完成后 rx 会自动关闭
        drop(tx);

        // 3. 调度循环
        while let Some(EngineMessage::RunNode(node_id, is_active, tx)) = rx.recv().await {
            let node = node_map.get(&node_id).cloned().unwrap();
            let state_lock = state_lock.clone();
            let adjacency = adjacency.clone();
            let ctx = ctx.clone();

            tokio::spawn(async move {
                // 优化：将是否执行业务逻辑的判断前置
                // 只有当 is_active 为 true 时才真正执行 executor
                let output_handle = if is_active {
                    let engine = WorkflowEngine::global();
                    if let Some(executor) = engine.get_executor(&node.node_type) {
                        match executor.execute(&ctx, &node.data).await {
                            Ok(output) => output.next_handle,
                            Err(e) => {
                                log::error!(
                                    "Node execution failed: node_id={}, error={}",
                                    node_id,
                                    e
                                );
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    // 节点未激活（被跳过），不产生输出 handle
                    None
                };

                // 4. 传播状态到子节点
                if let Some(edges) = adjacency.get(&node_id) {
                    let mut states = state_lock.lock().await;

                    for edge in edges {
                        if let Some(child_state) = states.get_mut(&edge.target) {
                            // 只有当当前节点激活且执行成功，才尝试激活子节点
                            if is_active {
                                // 检查 Handle 匹配逻辑
                                let condition_met = match &edge.source_handle {
                                    None => true, // 无特定 Handle 要求，默认流转
                                    Some(h) => Some(h) == output_handle.as_ref(),
                                };

                                if condition_met {
                                    child_state.is_active = true;
                                }
                            }

                            // 无论是否激活，入度都减一
                            child_state.in_degree -= 1;

                            // 当入度为 0 时，调度子节点
                            if child_state.in_degree == 0 {
                                let _ = tx
                                    .send(EngineMessage::RunNode(
                                        edge.target.clone(),
                                        child_state.is_active,
                                        tx.clone(),
                                    ))
                                    .await;
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }
}
