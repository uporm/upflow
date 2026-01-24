// 工作流数据模型定义
// 包括工作流、节点、边等核心数据结构
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

// 表示一个完整的工作流定义
// 包含工作流ID、节点列表和边列表
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub nodes: Vec<Node>,        // 节点列表
    pub edges: Vec<Edge>,        // 边列表，定义节点间的连接关系
}

// 表示工作流中的单个节点
// 可以是起始节点、决策节点、执行节点等不同类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,                             // 节点唯一标识符
    #[serde(default, alias = "parentId")]
    pub parent_id: Option<String>,              // 父节点ID，用于嵌套节点结构
    #[serde(rename = "type")]
    pub node_type: String,                      // 节点类型（如start, decision, end等）
    #[serde(default)]
    pub data: Arc<Value>,                       // 节点特定的数据
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,      // 重试策略
}

// 表示两个节点之间的连接关系
// 定义了数据流向和执行顺序
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,                       // 源节点ID
    pub target: String,                       // 目标节点ID
    #[serde(rename = "sourceHandle")]
    #[serde(default)]
    pub source_handle: Option<String>,        // 源句柄，用于多出口节点
}

// 工作流执行状态枚举
// 表示工作流的最终执行结果
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    Succeeded,        // 执行成功
    Failed,           // 执行失败
    Cancelled,        // 已取消
}

// 工作流执行结果结构体
// 包含执行实例ID、状态和输出结果
#[derive(Clone, Debug)]
pub struct WorkflowResult {
    pub instance_id: u64,           // 工作流实例唯一标识符
    pub status: FlowStatus,         // 执行状态
    pub output: Option<Value>,      // 执行输出结果
}

// 重试策略结构体
// 定义节点执行失败时的重试行为
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,        // 最大重试次数
    pub interval_ms: u64,         // 重试间隔（毫秒）
}
