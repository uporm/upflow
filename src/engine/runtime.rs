use crate::engine::graph::WorkflowGraph;
use crate::models::actor_message::ActorMessage;
use crate::models::context::{FlowContext, NodeContext};
use crate::models::error::WorkflowError;
use crate::models::event::WorkflowEvent;
use crate::models::event_bus::EventBus;
use crate::models::workflow::{FlowStatus, WorkflowResult};
use crate::nodes::NodeExecutor;
use chrono::Utc;
use dashmap::DashMap;
use petgraph::Direction;
use petgraph::visit::EdgeRef;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

/// 工作流调度器
/// 负责管理节点执行器并调度工作流的执行过程
pub struct WorkflowRuntime {
    node_executors: Arc<DashMap<String, Arc<dyn NodeExecutor>>>,
}

impl WorkflowRuntime {
    /// 创建一个新的调度器实例
    pub fn new() -> Self {
        Self {
            node_executors: Arc::new(DashMap::new()),
        }
    }

    /// 注册节点类型的执行器
    ///
    /// # 参数
    /// - `node_type`: 节点类型标识
    /// - `executor`: 具体的执行逻辑实现
    pub fn register(&self, node_type: &str, executor: impl NodeExecutor + 'static) {
        self.node_executors
            .insert(node_type.to_string(), Arc::new(executor));
    }

    /// 执行工作流
    ///
    /// # 参数
    /// - `flow_context`: 流程上下文，包含输入和变量
    /// - `event_bus`: 事件总线，用于发送工作流事件
    /// - `workflow_graph`: 工作流图结构
    /// - `instance_id`: 工作流实例ID
    /// - `cancel_rx`: 取消信号接收器
    pub async fn execute(
        &self,
        flow_context: Arc<FlowContext>,
        event_bus: EventBus,
        workflow_graph: Arc<WorkflowGraph>,
        instance_id: u64,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<WorkflowResult, WorkflowError> {
        // 使用无界通道避免死锁，符合 Actor 模型常见实践
        let (tx, rx) = mpsc::unbounded_channel::<ActorMessage>();

        let mut actor = WorkflowActor::new(
            workflow_graph,
            self.node_executors.clone(),
            flow_context,
            event_bus,
            tx,
            instance_id,
            cancel_rx,
        );

        actor.run(rx).await
    }
}

/// 工作流 Actor
/// 封装了工作流运行时的状态和消息处理逻辑
struct WorkflowActor {
    graph: Arc<WorkflowGraph>,
    executors: Arc<DashMap<String, Arc<dyn NodeExecutor>>>,
    flow_context: Arc<FlowContext>,
    event_bus: EventBus,
    tx: mpsc::UnboundedSender<ActorMessage>,
    remaining_dependencies: HashMap<String, usize>,
    pending_count: usize,
    active_incoming: HashMap<String, usize>,
    instance_id: u64,
    cancel_rx: watch::Receiver<bool>,
}

impl WorkflowActor {
    fn new(
        graph: Arc<WorkflowGraph>,
        executors: Arc<DashMap<String, Arc<dyn NodeExecutor>>>,
        flow_context: Arc<FlowContext>,
        event_bus: EventBus,
        tx: mpsc::UnboundedSender<ActorMessage>,
        instance_id: u64,
        cancel_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            graph,
            executors,
            flow_context,
            event_bus,
            tx,
            remaining_dependencies: HashMap::new(),
            active_incoming: HashMap::new(),
            pending_count: 0,
            instance_id,
            cancel_rx,
        }
    }

    /// 初始化工作流状态
    ///
    /// # 逻辑流程
    /// 1. 遍历图中的所有节点
    /// 2. 计算每个节点的入度（上游依赖的数量）
    /// 3. 将入度为0的节点加入初始节点列表
    /// 4. 将其他节点的依赖数量记录到remaining_dependencies中
    ///
    /// # 返回值
    /// - 返回入度为0的节点ID列表（即可以立即执行的节点）
    fn init_state(&mut self) -> Vec<String> {
        let mut initial_nodes = Vec::new();
        // 遍历工作流图中的所有节点
        for idx in self.graph.dag.node_indices() {
            let node = &self.graph.dag[idx];
            // 计算当前节点的入度（上游依赖数量）
            let incoming_count = self
                .graph
                .dag
                .edges_directed(idx, Direction::Incoming)
                .count();

            // 根据入度数量分类处理节点
            match incoming_count {
                0 => initial_nodes.push(node.id.clone()), // 入度为0的节点可以立即执行
                _ => {
                    // 其他节点需要等待上游依赖完成，记录剩余依赖数量
                    self.remaining_dependencies
                        .insert(node.id.clone(), incoming_count);
                }
            }
        }
        initial_nodes
    }

    /// 执行工作流的主要方法
    ///
    /// # 逻辑流程
    /// 1. 初始化工作流实例ID和状态
    /// 2. 发送工作流开始事件
    /// 3. 获取初始可执行节点列表
    /// 4. 验证图结构有效性（防止循环依赖或没有起始节点）
    /// 5. 根据初始节点数量决定是否并发执行
    /// 6. 向消息队列发送执行消息以启动初始节点
    /// 7. 处理节点间的消息传递直到所有节点完成
    /// 8. 收集最终结果并发送工作流结束事件
    ///
    /// # 参数
    /// - `rx`: 接收来自节点的消息通道
    ///
    /// # 返回值
    /// - 成功时返回包含结果的 WorkflowResult
    /// - 失败时返回 WorkflowError
    async fn run(
        &mut self,
        rx: mpsc::UnboundedReceiver<ActorMessage>,
    ) -> Result<WorkflowResult, WorkflowError> {
        let start_time = std::time::Instant::now();

        // 检查是否已被取消
        if *self.cancel_rx.borrow() {
            return Err(WorkflowError::Cancelled);
        }

        // 发送工作流开始事件，通知监听者工作流已启动
        let nodes = self.graph.topological_nodes()?;
        self.event_bus.emit(WorkflowEvent::FlowStarted {
            instance_id: self.instance_id.to_string(),
            payload: Arc::new(self.flow_context.payload.clone()),
            nodes: Arc::new(nodes),
            timestamp: Utc::now(),
        });

        // 让出 CPU 时间片，确保 FlowStarted 事件能被订阅者及时处理
        tokio::task::yield_now().await;

        // 查找所有入度为 0 的节点作为初始节点
        let initial_nodes = self.init_state();

        // 如果有多个初始节点，则使用并发执行
        let spawn_initial_nodes = initial_nodes.len() > 1;

        // 为每个初始节点创建执行消息并将其发送到消息队列
        for node_id in initial_nodes {
            self.tx
                .send(ActorMessage::Execute {
                    node_id,
                    spawn: spawn_initial_nodes,
                })
                .map_err(|e| WorkflowError::RuntimeError(e.to_string()))?;
            // 增加待处理节点计数
            self.pending_count += 1;
        }

        // 处理节点间的消息交互，直到所有节点完成或发生错误
        if let Err(e) = self.process_messages(rx).await {
            let duration_ms = start_time.elapsed().as_millis() as u64;

            if matches!(e, WorkflowError::Cancelled) {
                self.event_bus.emit(WorkflowEvent::FlowStopped {
                    instance_id: self.instance_id.to_string(),
                    timestamp: Utc::now(),
                });
            } else {
                // 如果处理消息时发生错误，发送失败事件并返回错误
                self.event_bus.emit(WorkflowEvent::FlowFinished {
                    output: None,
                    duration_ms,
                });
            }

            // 让出 CPU 时间片，确保事件能被订阅者及时处理
            tokio::task::yield_now().await;
            return Err(e);
        }

        // 从出度为 0 的节点获取最终输出结果
        let output = self.graph.dag.node_indices().find_map(|idx| {
            if self
                .graph
                .dag
                .edges_directed(idx, Direction::Outgoing)
                .next()
                .is_none()
            {
                let node = &self.graph.dag[idx];
                self.flow_context.get_result(&node.id)
            } else {
                None
            }
        });
        let output_value = output.as_ref().map(|value| value.as_ref().clone());

        // 发送工作流完成事件，通知监听者工作流已成功结束
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.event_bus.emit(WorkflowEvent::FlowFinished {
            output: output.clone(),
            duration_ms,
        });

        // 让出 CPU 时间片，确保 FlowFinished 事件能被订阅者及时处理
        tokio::task::yield_now().await;

        // 返回成功的工作流结果
        Ok(WorkflowResult {
            instance_id: self.instance_id,
            status: FlowStatus::Succeeded,
            output: output_value,
        })
    }

    /// 处理节点间的消息传递
    ///
    /// # 逻辑流程
    /// 1. 循环接收消息直到所有节点完成（pending_count为0）
    /// 2. 根据消息类型进行相应处理：
    ///    - Execute: 启动节点执行
    ///    - NodeCompleted: 节点成功完成，更新上下文并触发下游节点
    ///    - NodeSkipped: 节点被跳过，直接触发下游节点
    ///    - NodeFailed: 节点执行失败，立即返回错误
    ///
    /// # 参数
    /// - `rx`: 接收来自节点的消息通道
    ///
    /// # 返回值
    /// - 成功时返回 ()
    /// - 失败时返回 WorkflowError
    async fn process_messages(
        &mut self,
        mut rx: mpsc::UnboundedReceiver<ActorMessage>,
    ) -> Result<(), WorkflowError> {
        // 持续处理消息直到没有待处理的节点
        while self.pending_count > 0 {
            let msg = tokio::select! {
                _ = self.cancel_rx.changed() => {
                    if *self.cancel_rx.borrow() {
                        return Err(WorkflowError::Cancelled);
                    }
                    continue;
                }
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => msg,
                        None => break,
                    }
                }
            };

            match msg {
                ActorMessage::Execute { node_id, spawn } => {
                    // 处理执行消息，启动指定节点的执行
                    let node_id_for_error = node_id.clone();
                    spawn_node_execution(
                        node_id,
                        self.graph.clone(),
                        self.executors.clone(),
                        self.flow_context.clone(),
                        self.event_bus.clone(),
                        self.tx.clone(),
                        spawn,
                    )
                    .await
                    .map_err(|e| {
                        WorkflowError::RuntimeError(format!(
                            "Node {} failed to start: {}",
                            node_id_for_error, e
                        ))
                    })?;
                }
                ActorMessage::NodeCompleted { node_id, output } => {
                    // 节点完成执行，减少待处理计数
                    self.pending_count -= 1;
                    // 将节点的输出结果存储到流程上下文中
                    self.flow_context.set_result(&node_id, Arc::clone(&output));
                    // 触发下游节点的执行
                    self.dispatch_downstream(&node_id, Some(output.as_ref()))?;
                }
                ActorMessage::NodeSkipped { node_id } => {
                    // 节点被跳过执行，减少待处理计数
                    self.pending_count -= 1;
                    // 触发下游节点的执行（无输出）
                    self.dispatch_downstream(&node_id, None)?;
                }
                ActorMessage::NodeFailed { node_id, error } => {
                    // 节点执行失败，立即返回错误终止整个工作流
                    return Err(WorkflowError::RuntimeError(format!(
                        "Node {} failed: {}",
                        node_id, error
                    )));
                }
            }
        }
        Ok(())
    }

    /// 分发下游节点的执行任务
    ///
    /// # 逻辑流程
    /// 1. 获取当前节点的所有下游节点及其激活状态
    /// 2. 检查哪些下游节点已经满足执行条件（所有依赖已完成）
    /// 3. 决定是否需要并发执行（如果有多个活跃的下游节点）
    /// 4. 向消息队列发送执行或跳过消息
    ///
    /// # 参数
    /// - `node_id`: 当前完成的节点ID
    /// - `output`: 当前节点的输出值，如果为None则表示节点被跳过
    ///
    /// # 返回值
    /// - 成功时返回 ()
    /// - 失败时返回 WorkflowError
    fn dispatch_downstream(
        &mut self,
        node_id: &str,
        output: Option<&Value>,
    ) -> Result<(), WorkflowError> {
        // 获取当前节点的下游节点列表及它们的激活状态
        let downstream = self.get_downstream_nodes(node_id, output)?;
        let mut ready = Vec::new();
        // 检查每个下游节点是否已准备好执行
        for (target_id, is_active) in downstream {
            if self.can_execute_node(&target_id, is_active) {
                // 获取目标节点的活跃传入边计数
                let active_count = self.active_incoming.get(&target_id).copied().unwrap_or(0);
                ready.push((target_id, active_count));
            }
        }

        // 总是启用并发执行，避免阻塞 Actor
        let spawn = true;
        // 为每个就绪的下游节点发送执行或跳过消息
        for (target_id, active_count) in ready {
            let message = if active_count > 0 {
                // 发送执行消息
                ActorMessage::Execute {
                    node_id: target_id,
                    spawn,
                }
            } else {
                // 发送跳过消息
                ActorMessage::NodeSkipped { node_id: target_id }
            };
            self.tx
                .send(message)
                .map_err(|e| WorkflowError::RuntimeError(e.to_string()))?;
            // 更新待处理节点计数
            self.pending_count += 1;
        }
        Ok(())
    }

    fn get_downstream_nodes(
        &self,
        node_id: &str,
        output: Option<&Value>,
    ) -> Result<Vec<(String, bool)>, WorkflowError> {
        let node_idx = self.graph.node_map.get(node_id).ok_or_else(|| {
            WorkflowError::RuntimeError(format!("Node {} not found in map", node_id))
        })?;
        let mut result = Vec::new();

        for edge in self
            .graph
            .dag
            .edges_directed(*node_idx, Direction::Outgoing)
        {
            let target_node = &self.graph.dag[edge.target()];
            let edge_data = edge.weight();
            let is_active = match output {
                Some(out_val) => match &edge_data.source_handle {
                    Some(handle) => {
                        matches!(out_val, Value::Object(map) if map.contains_key(handle))
                    }
                    None => true,
                },
                None => false,
            };
            result.push((target_node.id.clone(), is_active));
        }
        Ok(result)
    }

    fn can_execute_node(&mut self, node_id: &str, is_incoming_active: bool) -> bool {
        if is_incoming_active {
            *self.active_incoming.entry(node_id.to_string()).or_default() += 1;
        }

        if let Some(count) = self.remaining_dependencies.get_mut(node_id) {
            *count -= 1;
            return *count == 0;
        }
        // Should not happen for nodes with dependencies
        false
    }

    #[allow(dead_code)]
    fn is_workflow_completed(&self) -> bool {
        self.pending_count == 0
    }
}

async fn spawn_node_execution(
    node_id: String,
    graph: Arc<WorkflowGraph>,
    executors: Arc<DashMap<String, Arc<dyn NodeExecutor>>>,
    flow_context: Arc<FlowContext>,
    event_bus: EventBus,
    tx: mpsc::UnboundedSender<ActorMessage>,
    spawn: bool,
) -> Result<(), WorkflowError> {
    let node_idx = graph
        .node_map
        .get(&node_id)
        .ok_or_else(|| WorkflowError::RuntimeError(format!("Node {} not found in map", node_id)))?;
    let node = &graph.dag[*node_idx];
    let node_type = node.node_type.clone();

    // 解析输入参数
    let resolved_input = flow_context
        .resolve_value(node.data.as_ref())
        .map_err(|e| {
            WorkflowError::RuntimeError(format!("Input resolution failed for {}: {}", node_id, e))
        })?;
    let resolved_data = Arc::new(resolved_input);

    let executor = executors
        .get(&node_type)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            WorkflowError::RuntimeError(format!("No executor for type: {}", node_type))
        })?;

    let ctx = NodeContext {
        node: node.clone(),
        flow_context: Arc::clone(&flow_context),
        event_bus: event_bus.clone(),
        resolved_data: Arc::clone(&resolved_data),
    };
    event_bus.emit(WorkflowEvent::NodeStarted {
        node_id: node_id.clone(),
        node_type: node_type.clone(),
        data: Arc::clone(&resolved_data),
    });
    // 让出 CPU 时间片，确保 NodeStarted 事件能被订阅者及时处理
    tokio::task::yield_now().await;

    if spawn {
        tokio::spawn(run_executor(
            executor,
            ctx,
            node_id,
            node_type,
            resolved_data,
            event_bus,
            tx,
        ));
    } else {
        run_executor(
            executor,
            ctx,
            node_id,
            node_type,
            resolved_data,
            event_bus,
            tx,
        )
        .await;
    }
    Ok(())
}

async fn run_executor(
    executor: Arc<dyn NodeExecutor>,
    ctx: NodeContext,
    node_id: String,
    node_type: String,
    node_data: Arc<Value>,
    event_bus: EventBus,
    tx: mpsc::UnboundedSender<ActorMessage>,
) {
    let start = std::time::Instant::now();
    match executor.execute(ctx).await {
        Ok(output) => {
            let duration = start.elapsed().as_millis() as u64;
            let output = Arc::new(output);
            event_bus.emit(WorkflowEvent::NodeCompleted {
                node_id: node_id.clone(),
                node_type: node_type.clone(),
                data: Arc::clone(&node_data),
                output: Arc::clone(&output),
                duration_ms: duration,
            });
            // 让出 CPU 时间片，确保 NodeCompleted 事件能被订阅者及时处理
            tokio::task::yield_now().await;
            let _ = tx.send(ActorMessage::NodeCompleted { node_id, output });
        }
        Err(e) => {
            event_bus.emit(WorkflowEvent::NodeError {
                node_id: node_id.clone(),
                node_type: node_type.clone(),
                data: Arc::clone(&node_data),
                error: e.to_string(),
                strategy: "fail".to_string(),
            });
            // 让出 CPU 时间片，确保 NodeError 事件能被订阅者及时处理
            tokio::task::yield_now().await;
            let _ = tx.send(ActorMessage::NodeFailed {
                node_id,
                error: e.to_string(),
            });
        }
    }
}
