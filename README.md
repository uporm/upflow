# Upflow

Upflow 是一个基于 Rust 构建的强大、异步工作流引擎。它利用有向无环图（DAG）结构来编排复杂的任务依赖关系，支持并行执行、条件分支、子流程和自定义节点扩展。

Upflow 基于 `tokio` 构建，专为高性能和可扩展性而设计，非常适合构建编排平台、业务流程自动化和数据处理管道。

## 特性

- **基于 DAG 的编排**：使用有向无环图结构定义复杂的工作流。
- **异步执行**：基于 `tokio` 构建的全异步运行时，实现高效的资源利用。
- **可扩展的节点系统**：
  - **内置节点**：开始节点、决策节点（Switch）、子流程节点、分组节点。
  - **自定义节点**：通过实现 `NodeExecutor` trait 轻松实现自定义逻辑。
- **灵活的工作流定义**：工作流使用 JSON 定义，易于生成、存储和版本控制。
- **动态变量解析**：支持在节点之间动态解析变量（例如 `{{node_id.output_field}}`）。
- **事件驱动**：包含用于工作流生命周期事件的内部事件总线。

## 安装

将 `upflow` 添加到你的 `Cargo.toml`：

```toml
[dependencies]
upflow = { version = "0.2.0" } # 请检查 crates.io 获取最新版本
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
async-trait = "0.1"
```

## 快速开始

下面是一个简单的示例，展示如何定义工作流并使用 Upflow 运行它。

### 1. 定义工作流 (JSON)

创建一个描述 DAG 的 `workflow.json` 文件：

```json
{
  "nodes": [
    {
      "id": "node-start",
      "type": "start",
      "data": {
        "input": [
          { "name": "message", "type": "STRING" }
        ]
      }
    },
    {
      "id": "node-process",
      "type": "my-custom-node",
      "data": {
        "prefix": "Processed: "
      }
    },
    {
      "id": "node-output",
      "type": "output",
      "data": {
        "final_result": "{{node-process.result}}"
      }
    }
  ],
  "edges": [
    { "source": "node-start", "target": "node-process" },
    { "source": "node-process", "target": "node-output" }
  ]
}
```

### 2. 实现自定义节点并运行引擎

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
        // 使用预解析的输入数据
        // 上下文中的 resolved_data 已经是解析后的 Value (Arc<Value>)
        let input_data = &ctx.resolved_data;
        let prefix = input_data["prefix"].as_str().unwrap_or("");
        
        // 获取全局 payload
        // 访问流程上下文中的 payload 字段
        let payload = &ctx.flow_context.payload;
        let message = payload["message"].as_str().unwrap_or("default");

        let result = format!("{}{}", prefix, message);
        println!("Executing MyCustomNode: {}", result);

        // 返回输出
        Ok(json!({ "result": result }))
    }
}

// 简单的输出节点
struct OutputNode;

#[async_trait]
impl NodeExecutor for OutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = &ctx.resolved_data;
        println!("Workflow Final Output: {:?}", resolved);
        Ok(resolved.as_ref().clone())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化引擎
    let engine = WorkflowEngine::global();

    // 2. 注册节点
    engine.register("my-custom-node", MyCustomNode);
    engine.register("output", OutputNode);

    // 3. 加载工作流
    let workflow_json = include_str!("workflow.json"); // 假设 workflow.json 在 src/ 目录下或与 main 同级
    engine.load("my-workflow", workflow_json)?;

    // 4. 运行工作流
    let payload = json!({ "message": "Hello World" });
    let ctx = Arc::new(FlowContext::new().with_payload(payload));
    
    let result = engine.run_with_ctx("my-workflow", ctx).await?;

    println!("Workflow Status: {:?}", result.status);
    Ok(())
}
```

## 核心概念

- **Workflow (工作流)**：由节点和边组成的有向无环图 (DAG)。
- **Node (节点)**：工作的原子单元。Upflow 支持多种节点类型，你也可以创建自己的节点。
- **Edge (边)**：定义两个节点之间的依赖关系。
- **FlowContext (流程上下文)**：保存工作流执行的状态，包括全局 payload 和已执行节点的输出。
- **Engine (引擎)**：管理工作流加载、验证和执行的核心组件。

## 贡献

欢迎贡献！如果你有改进的想法或发现了 bug，请开启 issue 或提交 pull request。

1. Fork 本仓库。
2. 创建你的特性分支 (`git checkout -b feature/amazing-feature`)。
3. 提交你的更改 (`git commit -m 'Add some amazing feature'`)。
4. 推送到分支 (`git push origin feature/amazing-feature`)。
5. 开启 Pull Request。

## 许可证

本项目采用 Apache-2.0 许可证。
