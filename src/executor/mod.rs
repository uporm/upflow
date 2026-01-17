use crate::model::WorkflowDefinition;
use crate::context::WorkflowContext;
use crate::engine::WorkflowEngine;
use crate::error::WorkflowError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct WorkflowRunner {
    definition: Arc<WorkflowDefinition>,
}

enum EngineMessage {
    RunNode(String, mpsc::Sender<EngineMessage>),
}

impl WorkflowRunner {
    pub fn new(def: Arc<WorkflowDefinition>) -> Self {
        Self { definition: def }
    }

    pub async fn run(&self, ctx: Arc<WorkflowContext>) -> Result<(), WorkflowError> {
        // 0. 校验所有节点类型是否有对应的 Executor
        let engine = WorkflowEngine::global();
        for node in &self.definition.nodes {
            if engine.get_executor(&node.node_type).is_none() {
                return Err(WorkflowError::RuntimeError(format!("Unknown node type: {}", node.node_type)));
            }
        }

        let mut in_degree = HashMap::new();
        let mut adjacency = HashMap::new();

        // 1. 初始化 DAG
        for node in &self.definition.nodes {
            in_degree.insert(node.id.clone(), 0);
        }
        for edge in &self.definition.edges {
            *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
            adjacency.entry(edge.source.clone()).or_insert(vec![]).push(edge.clone());
        }

        let in_degree = Arc::new(Mutex::new(in_degree));
        let active_nodes = Arc::new(Mutex::new(HashSet::new()));
        let (tx, mut rx) = mpsc::channel::<EngineMessage>(100);

        // 2. 启动入口节点
        {
            let degree = in_degree.lock().await;
            let mut active = active_nodes.lock().await;
            for (id, &d) in degree.iter() {
                if d == 0 { 
                    active.insert(id.clone());
                    tx.send(EngineMessage::RunNode(id.clone(), tx.clone())).await.unwrap(); 
                }
            }
        }
        
        // 显式 drop tx，这样当所有任务完成后 rx 会关闭
        drop(tx);

        // 3. 调度循环
        while let Some(EngineMessage::RunNode(node_id, tx)) = rx.recv().await {
            let node_cfg = self.definition.nodes.iter().find(|n| n.id == node_id).cloned().unwrap();

            let in_degree_clone = in_degree.clone();
            let active_nodes_clone = active_nodes.clone();
            let ctx_clone = ctx.clone();
            let edges = adjacency.get(&node_id).cloned().unwrap_or_default();

            tokio::spawn(async move {
                let engine = WorkflowEngine::global();
                
                // 检查节点是否激活
                let is_active = {
                    let active = active_nodes_clone.lock().await;
                    active.contains(&node_id)
                };

                if is_active {
                    if let Some(executor) = engine.get_executor(&node_cfg.node_type) {
                        let result = executor.execute(&ctx_clone, &node_cfg.data).await;

                        let mut degree_lock = in_degree_clone.lock().await;
                        match result {
                            Ok(output) => {
                                for edge in edges {
                                    // 检查 handle 匹配
                                    let mut match_handle = true;
                                    if let Some(ref h) = edge.source_handle {
                                        if Some(h) != output.next_handle.as_ref() {
                                            match_handle = false;
                                        }
                                    }
                                    
                                    if match_handle {
                                        // 激活目标节点
                                        let mut active = active_nodes_clone.lock().await;
                                        active.insert(edge.target.clone());
                                    }
                                    
                                    Self::decrement_degree(&edge.target, &mut degree_lock, &tx).await;
                                }
                            }
                            Err(e) => {
                                log::error!("Node execution failed: node_id={}, error={}", node_id, e);
                            }
                        }
                    }
                } else {
                    // 节点未激活（被跳过），传播跳过状态
                    let mut degree_lock = in_degree_clone.lock().await;
                    for edge in edges {
                        Self::decrement_degree(&edge.target, &mut degree_lock, &tx).await;
                    }
                }
            });
        }
        Ok(())
    }

    async fn decrement_degree(target: &str, degree_map: &mut HashMap<String, i32>, tx: &mpsc::Sender<EngineMessage>) {
        if let Some(d) = degree_map.get_mut(target) {
            *d -= 1;
            if *d == 0 { 
                tx.send(EngineMessage::RunNode(target.to_string(), tx.clone())).await.unwrap(); 
            }
        }
    }
}
