use crate::nodes::{NodeExecutor, NodeOutput};
use crate::context::WorkflowContext;
use crate::engine::WorkflowEngine;
use crate::error::WorkflowError;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct SubflowNode;

const MAX_DEPTH: u32 = 5; // 生产环境建议配置最大嵌套深度

#[async_trait]
impl NodeExecutor for SubflowNode {
    async fn execute(&self, ctx: &WorkflowContext, data: &Value) -> Result<NodeOutput, WorkflowError> {
        let sub_flow_id = data["flowId"].as_str()
            .ok_or_else(|| WorkflowError::ConfigError("SubflowNode missing 'flowId'".into()))?;

        // 1. 防死循环检查
        if ctx.depth >= MAX_DEPTH {
            return Err(WorkflowError::RuntimeError(format!("Max subflow depth ({}) exceeded", MAX_DEPTH)));
        }

        // 2. 输入映射：从父流程提取数据传给子流程
        let mut sub_initial_vars = HashMap::new();
        if let Some(inputs) = data["input"].as_array() {
            for mapping in inputs {
                #[allow(clippy::collapsible_if)]
                if let (Some(name), Some(val_key)) = (mapping["name"].as_str(), mapping["value"].as_str()) {
                    if let Some(val) = ctx.get_var(val_key) {
                        sub_initial_vars.insert(name.to_string(), val);
                    }
                }
            }
        }

        // 3. 执行子流程
        // 我们利用单例引擎运行子流程，传递当前的深度 + 1
        println!("[Subflow] Triggering {} from parent run {}", sub_flow_id, ctx.run_id);

        // 注意：这里需要引擎暴露一个 run_internal 或者类似方法，支持自定义 Context 初始化
        let sub_ctx = Arc::new(WorkflowContext::new(ctx.depth + 1));
        for (k, v) in sub_initial_vars {
            sub_ctx.set_var(k, v);
        }

        // 获取子流程定义并运行
        WorkflowEngine::global().run_with_context(sub_flow_id, sub_ctx.clone()).await?;

        // 4. 输出映射：从子流程结果写回父流程
        let result_vars = HashMap::new();
        // 此处逻辑根据业务需求，将 sub_ctx 中的变量映射回 NodeOutput

        Ok(NodeOutput {
            next_handle: None,
            updated_vars: result_vars,
        })
    }
}