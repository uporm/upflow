use crate::core::enums::NodeType;
use crate::engine::graph::WorkflowGraph;
use crate::engine::group_extractor::split_workflow_for_groups;
use crate::engine::runtime::WorkflowRuntime;
use crate::models::context::FlowContext;
use crate::models::error::WorkflowError;
use crate::models::event_bus::EventBus;
use crate::models::workflow::{Workflow, WorkflowResult};
use crate::nodes::{DecisionNode, GroupNode, NodeExecutor, StartNode, SubflowNode};
use crate::utils::id::Id;
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use tokio::sync::watch;

static INSTANCE: OnceLock<WorkflowEngine> = OnceLock::new();

// 全局工作流引擎
// 管理所有工作流定义、运行时和执行状态
pub struct WorkflowEngine {
    workflow_graphs: DashMap<String, Arc<WorkflowGraph>>, // 工作流ID到图结构的映射
    workflow_runtime: WorkflowRuntime,                    // 工作流运行时环境
    workflow_hashes: DashMap<String, String>,             // 工作流ID到其内容哈希值的映射
    subflows_map: DashMap<String, Vec<String>>,           // 主工作流ID到其子工作流ID列表的映射
    cancellation_senders: DashMap<String, watch::Sender<bool>>, // 运行实例ID到取消信号发送器的映射
}

impl WorkflowEngine {
    // 获取全局工作流引擎实例
    // 使用单例模式确保整个应用中只有一个引擎实例
    pub fn global() -> &'static WorkflowEngine {
        INSTANCE.get_or_init(|| {
            let runtime = WorkflowRuntime::new();
            // 注册内置节点执行器
            runtime.register(NodeType::Start.to_str(), StartNode);
            runtime.register(NodeType::Decision.to_str(), DecisionNode);
            runtime.register(NodeType::Subflow.to_str(), SubflowNode);
            runtime.register(NodeType::Group.to_str(), GroupNode);

            WorkflowEngine {
                workflow_graphs: DashMap::new(),
                workflow_runtime: runtime, // 初始化运行时环境
                workflow_hashes: DashMap::new(),
                subflows_map: DashMap::new(),
                cancellation_senders: DashMap::new(),
            }
        })
    }


    pub fn register(&self, node_type: &str, executor: impl NodeExecutor + 'static) {
        self.workflow_runtime.register(node_type, executor);
    }

    // 加载工作流定义到引擎中
    // 解析JSON格式的工作流定义，构建内部图结构，并处理子工作流
    pub fn load(&self, workflow_id: &str, json: &str) -> Result<(), WorkflowError> {
        // 如果工作流哈希值相同，则无需重新加载工作流
        let hash = compute_hash(json);
        if let Some(existing) = self.workflow_hashes.get(workflow_id) {
            if *existing == hash {
                return Ok(());
            } else if let Some((_, subflow_ids)) = self.subflows_map.remove(workflow_id) {
                // 清理旧的子工作流
                for subflow_id in subflow_ids {
                    self.workflow_graphs.remove(&subflow_id);
                }
            }
        }

        // 解析工作流定义
        let workflow: Workflow =
            serde_json::from_str(json).map_err(|e| WorkflowError::ParseError(e.to_string()))?;
        let (main_workflow, subflows) = split_workflow_for_groups(workflow_id, workflow);

        // 构建主工作流图
        self.build_workflow_graph(workflow_id, main_workflow)?;
        // 构建子工作流图
        for (subflow_id, subflow) in subflows {
            self.build_workflow_graph(&subflow_id, subflow)?;
            // 记录主工作流和子工作流的关系
            self.subflows_map
                .entry(workflow_id.to_string())
                .or_default()
                .push(subflow_id.to_string());
        }
        // 保存工作流哈希值用于后续比较
        self.workflow_hashes.insert(workflow_id.to_string(), hash);

        Ok(())
    }

    // 运行指定的工作流
    // 使用默认上下文和事件总线执行工作流
    pub async fn run(&self, workflow_id: &str) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(
            workflow_id,
            Arc::new(FlowContext::new()),
            EventBus::default(),
        )
        .await
    }

    // 使用指定的事件总线运行工作流
    // 使用默认上下文但自定义事件总线
    pub async fn run_with_event(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(workflow_id, Arc::new(FlowContext::new()), event_bus)
            .await
    }

    // 使用指定的上下文运行工作流
    // 使用自定义上下文但默认事件总线
    pub async fn run_with_ctx(
        &self,
        workflow_id: &str,
        flow_context: Arc<FlowContext>,
    ) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(workflow_id, flow_context, EventBus::default())
            .await
    }

    // 使用指定的上下文和事件总线运行工作流
    // 这是实际执行工作流的核心方法
    pub async fn run_with_ctx_event(
        &self,
        workflow_id: &str,
        flow_context: Arc<FlowContext>,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        // 生成实例ID
        let instance_id = Id::next_id()?;
        self.run_with_instance_id(instance_id, workflow_id, flow_context, event_bus).await
    }

    // 使用指定的实例ID运行工作流
    // 允许调用者控制实例ID，以便在运行时终止工作流
    pub async fn run_with_instance_id(
        &self,
        instance_id: u64,
        workflow_id: &str,
        flow_context: Arc<FlowContext>,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        // 获取对应的工作流图
        let graph = {
            self.workflow_graphs
                .get(workflow_id)
                .map(|entry| entry.value().clone())
                .ok_or_else(|| WorkflowError::WorkflowNotFound(workflow_id.to_string()))?
        };

        let instance_id_str = instance_id.to_string();

        // 创建取消信号通道
        let (tx, rx) = watch::channel(false);
        self.cancellation_senders.insert(instance_id_str.clone(), tx);

        // 通过运行时环境执行工作流
        let result = self.workflow_runtime
            .execute(flow_context, event_bus, graph, instance_id, rx)
            .await;

        // 清理取消信号发送器
        self.cancellation_senders.remove(&instance_id_str);

        result
    }

    // 手动终止指定的工作流实例
    pub fn stop(&self, instance_id: &str) -> bool {
        if let Some(sender) = self.cancellation_senders.get(instance_id) {
            let _ = sender.send(true);
            true
        } else {
            false
        }
    }

    // 构建工作流图并将其保存到缓存中
    // 将工作流定义转换为内部图结构并缓存
    fn build_workflow_graph(
        &self,
        workflow_id: &str,
        workflow: Workflow,
    ) -> Result<(), WorkflowError> {
        let mut graph = WorkflowGraph::new();
        graph.build(workflow)?; // 构建图结构并验证
        // 将图结构包装在Arc中并插入缓存
        self.workflow_graphs
            .insert(workflow_id.to_string(), Arc::new(graph));
        Ok(())
    }
}

// 计算字符串的SHA256哈希值
// 用于检测工作流定义是否发生变化
fn compute_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}
