# Upflow

Upflow 是一个基于 Rust 构建的强大、异步工作流引擎。它利用有向无环图（DAG）结构来编排复杂的任务依赖关系，支持并行执行、条件分支、子流程和自定义节点扩展。

Upflow 基于 `tokio` 构建，专为高性能和可扩展性而设计，非常适合构建编排平台、业务流程自动化和数据处理管道。

## 核心特性

- **基于 DAG 的编排**：使用有向无环图结构定义复杂的工作流，自动处理依赖关系。
- **高性能异步执行**：基于 `tokio` 全异步运行时，支持高并发任务处理。
- **丰富的节点类型**：
  - **内置节点**：开始节点 (`start`)、决策节点 (`decision`)、子流程节点 (`subflow`)、分组节点 (`group`)。
  - **自定义节点**：通过实现 `NodeExecutor` trait 轻松扩展业务逻辑。
- **灵活的工作流定义**：使用 JSON 格式定义工作流，易于生成、存储、版本控制和前端可视化。
- **强大的变量系统**：支持动态变量解析（如 `{{node_id.output_field}}`、`{{sys.key}}`），实现节点间数据传递。
- **事件驱动架构**：内置事件总线，支持工作流生命周期事件监听和自定义消息传递。
- **嵌套与复用**：支持子流程（Subflow）和分组（Group），实现复杂逻辑的模块化和复用。

## 安装

在你的 `Cargo.toml` 中添加 `upflow`：

```toml
[dependencies]
upflow = { version = "0.3.1" } # 请检查 crates.io 获取最新版本
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
async-trait = "0.1"
```

## 快速开始

以下示例展示了如何定义一个简单的工作流并运行它。

### 1. 定义工作流 (JSON)

创建一个 `workflow.json` 文件描述工作流结构：

```json
{
  "nodes": [
    {
      "id": "node-start",
      "type": "start",
      "data": { "input": [{ "name": "user_id", "type": "STRING" }] }
    },
    {
      "id": "node-process",
      "type": "my-custom-node",
      "data": { "prefix": "Hello, User " }
    },
    {
      "id": "node-decision",
      "type": "decision",
      "data": {
        "cases": [
          {
            "conditions": [
              { "var": "{{node-process.result}}", "opr": "contains", "value": "Admin" }
            ],
            "handle": "admin_path"
          }
        ],
        "else": { "handle": "default_path" }
      }
    }
  ],
  "edges": [
    { "source": "node-start", "target": "node-process" },
    { "source": "node-process", "target": "node-decision" }
  ]
}
```

### 2. 实现自定义节点并运行

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use upflow::prelude::*;

// 定义自定义节点
struct MyCustomNode;

#[async_trait]
impl NodeExecutor for MyCustomNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        // 获取解析后的输入数据
        let input_data = &ctx.resolved_data;
        let prefix = input_data["prefix"].as_str().unwrap_or("");
        
        // 获取流程上下文中的 payload
        let payload = &ctx.flow_context.payload;
        let user_id = payload["user_id"].as_str().unwrap_or("Guest");

        let result = format!("{}{}", prefix, user_id);
        println!("Executing MyCustomNode: {}", result);

        // 返回执行结果
        Ok(json!({ "result": result }))
    }
}

#[tokio::main]
async fn main() -> Result<(), WorkflowError> {
    // 1. 获取引擎实例
    let engine = WorkflowEngine::global();

    // 2. 注册自定义节点
    engine.register("my-custom-node", MyCustomNode);

    // 3. 加载工作流定义 (此处仅为示例字符串，实际可从文件读取)
    let workflow_json = r#"{...}"#; // 使用上面的 JSON 内容
    engine.load("my-workflow", workflow_json)?;

    // 4. 准备初始数据 (Payload)
    let payload = json!({ "user_id": "Admin123" });
    let context = Arc::new(FlowContext::new().with_payload(payload));

    // 5. 运行工作流
    let result = engine.run_with_ctx_event("my-workflow", context, EventBus::default()).await?;

    println!("Workflow execution finished. Status: {:?}", result.status);
    Ok(())
}
```

## 核心概念

### 节点类型 (Node Types)

- **Start (`start`)**: 工作流的入口点，通常用于定义输入参数。
- **Decision (`decision`)**: 条件分支节点。
  - 支持 `and`/`or` 逻辑组合。
  - 操作符 (`opr`) 支持：`eq`, `ne`, `gt`, `ge`, `lt`, `le`, `in`, `contains`。
  - 根据条件匹配结果，流程将走向不同的 `handle`（路径）。
- **Subflow (`subflow`)**: 子流程节点。
  - 执行另一个独立的工作流 (`subflowId`)。
  - 拥有独立的 `FlowContext`，数据隔离。
- **Group (`group`)**: 分组节点。
  - 执行另一个工作流 (`groupFlowId`) 作为当前流程的一部分。
  - 共享当前的 `FlowContext`，适合逻辑复用但需要共享数据的场景。
- **Custom**: 用户自定义节点，实现 `NodeExecutor` trait。

### 上下文 (Context)

- **FlowContext**: 贯穿整个工作流执行周期的上下文。
  - `payload`: 初始输入数据。
  - `env`: 环境变量（支持 `session.` 开头的变量动态更新）。
  - `node_results`: 存储所有节点的执行结果。
- **NodeContext**: 单个节点执行时的上下文。
  - `resolved_data`: 经过变量解析后的节点配置数据。
  - `node`: 节点元数据（ID、类型等）。

### 变量解析 (Variables)

Upflow 支持在节点配置中使用 `{{...}}` 语法引用变量：

- **引用节点输出**: `{{node_id.field}}` (例如 `{{step1.result}}`)
- **引用 Payload**: `{{payload.field}}` (例如 `{{payload.user_id}}`)
- **引用环境变量**: `{{sys.env.key}}` (例如 `{{sys.env.API_KEY}}`)

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

本项目采用 Apache-2.0 许可证。
