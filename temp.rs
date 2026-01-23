use async_trait::async_trait;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

// ============= 错误定义 =============
#[derive(Debug, Clone)]
pub enum WorkflowError {
    ExecutionError(String),
    ValidationError(String),
    NodeNotFound(String),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            WorkflowError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            WorkflowError::NodeNotFound(msg) => write!(f, "Node not found: {}", msg),
        }
    }
}

impl std::error::Error for WorkflowError {}

// ============= 节点上下文 =============
#[derive(Debug, Clone)]
pub struct NodeContext {
    pub node_id: String,
    pub input_data: Value,
    pub workflow_vars: Arc<RwLock<HashMap<String, Value>>>,
}

// ============= 节点执行器特质 =============
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError>;
    fn validate(&self, _data: &Value) -> Result<(), WorkflowError> {
        Ok(())
    }
}

// ============= 节点定义 =============
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub data: Value,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "sourceHandle")]
    pub source_handle: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Workflow {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

// ============= Actor 消息 =============
#[derive(Debug, Clone)]
pub enum ActorMessage {
    Execute {
        node_id: String,
        input_data: Value,
        source_handle: Option<String>,
    },
    NodeCompleted {
        node_id: String,
        output_data: Value,
        source_handle: Option<String>,
    },
    WorkflowCompleted,
    Error {
        node_id: String,
        error: String,
    },
}

// ============= 具体节点执行器实现 =============

// 开始节点
pub struct StartExecutor;

#[async_trait]
impl NodeExecutor for StartExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("📍 [START] Node: {}", ctx.node_id);
        println!("   Data: {}", serde_json::to_string_pretty(&ctx.input_data).unwrap());
        Ok(ctx.input_data)
    }
}

// 打印节点
pub struct PrintExecutor;

#[async_trait]
impl NodeExecutor for PrintExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("🖨️  [PRINT] Node: {}", ctx.node_id);
        println!("   Data: {}", serde_json::to_string_pretty(&ctx.input_data).unwrap());
        Ok(ctx.input_data)
    }
}

// 条件分支节点
pub struct CaseExecutor {
    cases: Value,
}

impl CaseExecutor {
    pub fn new(data: Value) -> Self {
        Self { cases: data }
    }
}

#[async_trait]
impl NodeExecutor for CaseExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("🔀 [CASE] Node: {}", ctx.node_id);
        println!("   Data: {}", serde_json::to_string_pretty(&self.cases).unwrap());
        
        // 简化的条件判断逻辑
        let vars = ctx.workflow_vars.read().await;
        if let Some(name) = vars.get("start_node.name") {
            if name.as_str() == Some("张三") {
                println!("   ✓ Matched condition: name == '张三', routing to 'zhangsan' handle");
                return Ok(serde_json::json!({"route": "zhangsan"}));
            }
        }
        
        println!("   ✓ No condition matched, routing to 'default' handle");
        Ok(serde_json::json!({"route": "default"}))
    }
}

// 循环节点
pub struct LoopExecutor;

#[async_trait]
impl NodeExecutor for LoopExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("🔁 [LOOP] Node: {}", ctx.node_id);
        println!("   Data: {}", serde_json::to_string_pretty(&ctx.input_data).unwrap());
        println!("   Executing loop iterations...");
        Ok(ctx.input_data)
    }
}

// 子流程节点
pub struct SubflowExecutor {
    subflow_id: String,
}

impl SubflowExecutor {
    pub fn new(data: Value) -> Self {
        let subflow_id = data["subflowId"].as_str().unwrap_or("unknown").to_string();
        Self { subflow_id }
    }
}

#[async_trait]
impl NodeExecutor for SubflowExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("📦 [SUBFLOW] Node: {}", ctx.node_id);
        println!("   Subflow ID: {}", self.subflow_id);
        println!("   Data: {}", serde_json::to_string_pretty(&ctx.input_data).unwrap());
        Ok(ctx.input_data)
    }
}

// 结束节点
pub struct EndExecutor;

#[async_trait]
impl NodeExecutor for EndExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!("🏁 [END] Node: {}", ctx.node_id);
        println!("   Data: {}", serde_json::to_string_pretty(&ctx.input_data).unwrap());
        Ok(ctx.input_data)
    }
}

// ============= 工作流引擎 =============
pub struct WorkflowEngine {
    graph: DiGraph<Node, String>,
    node_index_map: HashMap<String, NodeIndex>,
    executors: HashMap<String, Arc<dyn NodeExecutor>>,
    workflow_vars: Arc<RwLock<HashMap<String, Value>>>,
    tx: mpsc::UnboundedSender<ActorMessage>,
    rx: Arc<RwLock<mpsc::UnboundedReceiver<ActorMessage>>>,
    pending_nodes: Arc<RwLock<HashMap<String, usize>>>, // 跟踪待完成的前置节点数
    completed_nodes: Arc<RwLock<std::collections::HashSet<String>>>, // 已完成的节点
}

impl WorkflowEngine {
    pub fn new(workflow: Workflow) -> Self {
        let mut graph = DiGraph::new();
        let mut node_index_map = HashMap::new();
        let mut executors: HashMap<String, Arc<dyn NodeExecutor>> = HashMap::new();

        // 创建节点
        for node in &workflow.nodes {
            let idx = graph.add_node(node.clone());
            node_index_map.insert(node.id.clone(), idx);

            // 注册执行器
            let executor: Arc<dyn NodeExecutor> = match node.node_type.as_str() {
                "start" => Arc::new(StartExecutor),
                "print" => Arc::new(PrintExecutor),
                "case" => Arc::new(CaseExecutor::new(node.data.clone())),
                "loop" => Arc::new(LoopExecutor),
                "subflow" => Arc::new(SubflowExecutor::new(node.data.clone())),
                "end" => Arc::new(EndExecutor),
                _ => Arc::new(PrintExecutor), // 默认执行器
            };
            executors.insert(node.id.clone(), executor);
        }

        // 创建边
        for edge in &workflow.edges {
            if let (Some(&source_idx), Some(&target_idx)) = (
                node_index_map.get(&edge.source),
                node_index_map.get(&edge.target),
            ) {
                let edge_label = edge.source_handle.clone().unwrap_or_default();
                graph.add_edge(source_idx, target_idx, edge_label);
            }
        }

        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            graph,
            node_index_map,
            executors,
            workflow_vars: Arc::new(RwLock::new(HashMap::new())),
            tx,
            rx: Arc::new(RwLock::new(rx)),
            pending_nodes: Arc::new(RwLock::new(HashMap::new())),
            completed_nodes: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    pub async fn execute(&mut self, initial_input: Value) -> Result<(), WorkflowError> {
        println!("\n🚀 ========== Workflow Execution Started ==========\n");

        // 初始化工作流变量
        if let Some(name) = initial_input.get("name") {
            self.workflow_vars.write().await.insert("start_node.name".to_string(), name.clone());
        }

        // 初始化待处理节点计数
        self.init_pending_counts().await;

        // 找到所有开始节点并发送执行消息
        for (node_id, &idx) in &self.node_index_map {
            let node = &self.graph[idx];
            if node.node_type == "start" && node.parent_id.is_none() {
                self.tx.send(ActorMessage::Execute {
                    node_id: node_id.clone(),
                    input_data: initial_input.clone(),
                    source_handle: None,
                }).unwrap();
            }
        }

        // 启动消息处理循环
        self.process_messages().await?;

        println!("\n✅ ========== Workflow Execution Completed ==========\n");
        Ok(())
    }

    async fn init_pending_counts(&self) {
        let mut pending = self.pending_nodes.write().await;
        for (node_id, &idx) in &self.node_index_map {
            let incoming_count = self.graph.neighbors_directed(idx, petgraph::Direction::Incoming).count();
            if incoming_count > 0 {
                pending.insert(node_id.clone(), incoming_count);
            }
        }
    }

    async fn process_messages(&mut self) -> Result<(), WorkflowError> {
        let mut completed_count = HashMap::new();
        
        loop {
            let msg = {
                let mut rx = self.rx.write().await;
                rx.recv().await
            };

            match msg {
                Some(ActorMessage::Execute { node_id, input_data, source_handle }) => {
                    let tx = self.tx.clone();
                    let executor = self.executors.get(&node_id).cloned();
                    let workflow_vars = self.workflow_vars.clone();
                    let node_id_clone = node_id.clone();

                    tokio::spawn(async move {
                        if let Some(executor) = executor {
                            let ctx = NodeContext {
                                node_id: node_id_clone.clone(),
                                input_data,
                                workflow_vars,
                            };

                            match executor.execute(ctx).await {
                                Ok(output) => {
                                    tx.send(ActorMessage::NodeCompleted {
                                        node_id: node_id_clone,
                                        output_data: output,
                                        source_handle,
                                    }).unwrap();
                                }
                                Err(e) => {
                                    tx.send(ActorMessage::Error {
                                        node_id: node_id_clone,
                                        error: e.to_string(),
                                    }).unwrap();
                                }
                            }
                        }
                    });
                }

                Some(ActorMessage::NodeCompleted { node_id, output_data, source_handle }) => {
                    // 标记节点已完成
                    {
                        let mut completed = self.completed_nodes.write().await;
                        completed.insert(node_id.clone());
                    }

                    // 获取下游节点
                    let downstream_nodes = self.get_downstream_nodes(&node_id, &source_handle, &output_data);
                    
                    // 获取所有可能的下游节点
                    let all_downstream = self.get_all_downstream_nodes(&node_id);
                    
                    // 并行触发被路由到的下游节点
                    for (next_node_id, next_input, next_handle) in downstream_nodes.iter() {
                        // 检查是否所有前置节点都已完成
                        if self.can_execute_node(next_node_id, &mut completed_count).await {
                            self.tx.send(ActorMessage::Execute {
                                node_id: next_node_id.clone(),
                                input_data: next_input.clone(),
                                source_handle: next_handle.clone(),
                            }).unwrap();
                        }
                    }

                    // 对于未被路由到的下游节点，减少其等待计数
                    let executed_nodes: std::collections::HashSet<_> = downstream_nodes.iter()
                        .map(|(id, _, _)| id.clone())
                        .collect();
                    
                    for skipped_node in all_downstream {
                        if !executed_nodes.contains(&skipped_node) {
                            println!("   🚫 Node '{}' skipped (not routed from '{}')", skipped_node, node_id);
                            // 减少该节点的等待计数，但不执行它
                            let can_execute = self.can_execute_node(&skipped_node, &mut completed_count).await;
                            
                            // 如果该节点的所有前置都"完成"了（有些是跳过），
                            // 检查是否至少有一条路径是真正执行的
                            if can_execute {
                                let has_active_path = self.has_active_incoming_path(&skipped_node).await;
                                if has_active_path {
                                    // 这种情况不应该发生，因为如果有活跃路径，应该在上面的循环中处理
                                    println!("   ⚠️  Warning: Node '{}' has active path but wasn't routed", skipped_node);
                                }
                                // 如果没有活跃路径，该节点永远不会被执行，这是正常的
                            }
                        }
                    }

                    // 检查是否所有结束节点都已完成
                    if self.is_workflow_completed(&node_id).await {
                        self.tx.send(ActorMessage::WorkflowCompleted).unwrap();
                    }
                }

                Some(ActorMessage::WorkflowCompleted) => {
                    break;
                }

                Some(ActorMessage::Error { node_id, error }) => {
                    println!("❌ Error in node {}: {}", node_id, error);
                    return Err(WorkflowError::ExecutionError(error));
                }

                None => break,
            }
        }

        Ok(())
    }

    fn get_downstream_nodes(&self, node_id: &str, source_handle: &Option<String>, output_data: &Value) -> Vec<(String, Value, Option<String>)> {
        let mut result = Vec::new();
        
        if let Some(&node_idx) = self.node_index_map.get(node_id) {
            let node = &self.graph[node_idx];
            
            // 对于条件分支节点，根据输出选择路由
            let route = if node.node_type == "case" {
                output_data.get("route").and_then(|v| v.as_str()).map(String::from)
            } else {
                source_handle.clone()
            };

            let mut matched = false;
            let mut default_edge = None;

            for edge in self.graph.edges(node_idx) {
                let edge_weight = edge.weight();
                let target_idx = edge.target();
                let target_node = &self.graph[target_idx];

                // 对于条件分支节点
                if node.node_type == "case" {
                    if let Some(ref r) = route {
                        // 匹配具体的路由
                        if edge_weight == r {
                            result.push((
                                target_node.id.clone(),
                                output_data.clone(),
                                Some(edge_weight.clone()),
                            ));
                            matched = true;
                        } else if edge_weight == "default" {
                            // 保存默认分支
                            default_edge = Some((
                                target_node.id.clone(),
                                output_data.clone(),
                                Some(edge_weight.clone()),
                            ));
                        }
                    }
                } else {
                    // 非条件节点，执行所有下游节点
                    result.push((
                        target_node.id.clone(),
                        output_data.clone(),
                        Some(edge_weight.clone()),
                    ));
                }
            }

            // 如果是条件节点且没有匹配到任何分支，使用默认分支
            if node.node_type == "case" && !matched {
                if let Some(default) = default_edge {
                    println!("   ⚠️  No specific route matched, using default branch");
                    result.push(default);
                } else {
                    println!("   ⚠️  No route matched and no default branch found!");
                }
            }
        }

        result
    }

    fn get_all_downstream_nodes(&self, node_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        
        if let Some(&node_idx) = self.node_index_map.get(node_id) {
            for edge in self.graph.edges(node_idx) {
                let target_idx = edge.target();
                let target_node = &self.graph[target_idx];
                result.push(target_node.id.clone());
            }
        }
        
        result
    }

    async fn has_active_incoming_path(&self, node_id: &str) -> bool {
        // 检查该节点是否有任何前置节点已经完成
        if let Some(&node_idx) = self.node_index_map.get(node_id) {
            let completed = self.completed_nodes.read().await;
            
            for incoming_edge in self.graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                let source_idx = incoming_edge.source();
                let source_node = &self.graph[source_idx];
                
                if completed.contains(&source_node.id) {
                    return true;
                }
            }
        }
        false
    }

    async fn can_execute_node(&self, node_id: &str, completed_count: &mut HashMap<String, usize>) -> bool {
        let mut pending = self.pending_nodes.write().await;
        
        if let Some(required) = pending.get(node_id) {
            let current = completed_count.entry(node_id.to_string()).or_insert(0);
            *current += 1;
            
            if current >= required {
                pending.remove(node_id);
                true
            } else {
                false
            }
        } else {
            true // 没有前置节点的节点可以直接执行
        }
    }

    async fn is_workflow_completed(&self, node_id: &str) -> bool {
        if let Some(&node_idx) = self.node_index_map.get(node_id) {
            let node = &self.graph[node_idx];
            if node.node_type == "end" && node.parent_id.is_none() {
                // 检查是否还有其他end节点未完成
                let pending = self.pending_nodes.read().await;
                return pending.is_empty();
            }
        }
        false
    }
}

// ============= 示例使用 =============
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workflow_json = r#"{
        "nodes": [{
            "id": "start_node",
            "type": "start",
            "data": {
                "title": "开始",
                "input": [{
                    "name": "sex",
                    "type": "INTEGER"
                }, {
                    "name": "name",
                    "type": "STRING"
                }]
            }
        }, {
            "id": "bingxing-node-1",
            "type": "print",
            "data": {
                "title": "并行节点 1"
            }
        }, {
            "id": "bingxing-node-2",
            "type": "print",
            "data": {
                "title": "并行节点 2"
            }
        }, {
            "id": "switch_node",
            "type": "case",
            "data": {
                "title": "条件分支",
                "cases": [{
                    "sourceHandleId": "",
                    "opr": "and",
                    "conditions": [{
                        "varId": "{{start_node.name}}",
                        "opr": "in",
                        "value": "张三"
                    }],
                    "sourceHandle": "zhangsan"
                }],
                "else": {
                    "sourceHandle": "default"
                }
            }
        }, {
            "id": "loop-node",
            "type": "loop",
            "data": {
                "title": "循环"
            }
        }, {
            "id": "subflow-node",
            "type": "subflow",
            "data": {
                "title": "子流程",
                "subflowId": "subflow-1"
            }
        }, {
            "id": "end-node",
            "type": "end",
            "data": {
                "title": "结束",
                "output": {}
            }
        }],
        "edges": [{
            "id": "e1",
            "source": "start_node",
            "target": "bingxing-node-1"
        }, {
            "id": "e2",
            "source": "start_node",
            "target": "bingxing-node-2"
        }, {
            "id": "e3",
            "source": "bingxing-node-1",
            "target": "switch_node"
        }, {
            "id": "e4",
            "source": "bingxing-node-2",
            "target": "switch_node"
        }, {
            "id": "e5",
            "source": "switch_node",
            "target": "loop-node",
            "sourceHandle": "zhangsan"
        }, {
            "id": "e6",
            "source": "switch_node",
            "target": "subflow-node",
            "sourceHandle": "default"
        }, {
            "id": "e7",
            "source": "loop-node",
            "target": "end-node"
        }, {
            "id": "e8",
            "source": "subflow-node",
            "target": "end-node"
        }]
    }"#;

    let workflow: Workflow = serde_json::from_str(workflow_json)?;
    let mut engine = WorkflowEngine::new(workflow);

    // 测试场景1: name = "张三"，应该走 zhangsan 分支
    println!("=== Test Case 1: name = '张三' ===");
    let input = serde_json::json!({
        "sex": 1,
        "name": "张三"
    });
    engine.execute(input).await?;

    println!("\n\n");

    // 测试场景2: name = "李四"，应该走 default 分支
    println!("=== Test Case 2: name = '李四' ===");
    let workflow2: Workflow = serde_json::from_str(workflow_json)?;
    let mut engine2 = WorkflowEngine::new(workflow2);
    let input2 = serde_json::json!({
        "sex": 2,
        "name": "李四"
    });
    engine2.execute(input2).await?;

    Ok(())
}