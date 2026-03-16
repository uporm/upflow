use crate::models::context::NodeContext;
use crate::models::error::WorkflowError;
use crate::nodes::NodeExecutor;
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::thread;

/// 开始节点执行器
/// 负责处理工作流的输入参数验证
pub struct StartNode;

/// 开始节点配置数据结构
#[derive(Debug, Deserialize)]
struct StartNodeData {
    /// 输入参数定义列表
    #[serde(default)]
    input: Vec<InputDef>,
}

/// 输入参数定义
#[derive(Debug, Deserialize)]
struct InputDef {
    /// 参数名称
    name: String,
    /// 参数类型
    #[serde(rename = "type")]
    typ: InputType,
    /// 验证规则列表
    #[serde(default)]
    rules: Vec<RuleConfig>,
}

/// 支持的输入参数类型
#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[allow(non_camel_case_types)]
enum InputType {
    // 基本类型
    STRING,
    INTEGER,
    LONG,
    DECIMAL,
    BOOLEAN,
    OBJECT,
    // 文件类型
    FILE_IMAGE,
    FILE_VIDEO,
    FILE_AUDIO,
    FILE_DOCUMENT,
    // 数组类型
    ARRAY,
    ARRAY_STRING,
    ARRAY_INTEGER,
    ARRAY_LONG,
    ARRAY_DECIMAL,
    ARRAY_BOOLEAN,
    ARRAY_OBJECT,
    ARRAY_FILE_IMAGE,
    ARRAY_FILE_VIDEO,
    ARRAY_FILE_AUDIO,
    ARRAY_FILE_DOCUMENT,
}

/// 验证规则配置
#[derive(Debug, Deserialize)]
struct RuleConfig {
    /// 规则类型 (required, length, min, max, enum, pattern, email, size)
    #[serde(rename = "type")]
    rule_type: String,
    /// 验证失败时的错误消息
    message: Option<String>,

    // 验证参数
    /// 最小值 (用于数值)
    min: Option<f64>,
    /// 最大值 (用于数值)
    max: Option<f64>,
    /// 数组大小 (用于数组)
    size: Option<usize>,
    /// 字符串长度 (用于字符串)
    length: Option<usize>,
    /// 正则表达式模式 (用于字符串)
    pattern: Option<String>,
    /// 枚举值列表
    #[serde(rename = "enum")]
    enum_values: Option<Vec<Value>>,
}

#[async_trait]
impl NodeExecutor for StartNode {
    /// 执行开始节点逻辑
    /// 1. 解析节点配置中的输入定义
    /// 2. 验证工作流上下文中的 payload 数据是否符合定义
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!(
            "StartNode [{}] 线程号: {:?}",
            ctx.node.id,
            thread::current().id()
        );

        // 1. 解析配置
        let config: StartNodeData = serde_json::from_value(ctx.node.data.as_ref().clone())
            .map_err(|e| WorkflowError::ParseError(format!("Invalid StartNode config: {}", e)))?;

        // 2. 验证 payload
        let payload = &ctx.flow_context.payload;

        // 如果定义了输入参数，payload 必须是一个对象
        if !config.input.is_empty() {
            if !payload.is_object() {
                return Err(WorkflowError::ValidationError(
                    "StartNode payload must be an object".to_string(),
                ));
            }

            // 逐个验证输入参数
            for input_def in &config.input {
                let val = payload.get(&input_def.name);
                validate_input(val, input_def)?;
            }
        }

        Ok(payload.clone())
    }
}

/// 验证单个输入参数
fn validate_input(val: Option<&Value>, def: &InputDef) -> Result<(), WorkflowError> {
    // 检查必填项
    let is_required = def.rules.iter().any(|r| r.rule_type == "required");

    if val.is_none() || val.unwrap().is_null() {
        if is_required {
            let msg = def
                .rules
                .iter()
                .find(|r| r.rule_type == "required")
                .and_then(|r| r.message.clone())
                .unwrap_or_else(|| format!("Field '{}' is required", def.name));
            return Err(WorkflowError::ValidationError(msg));
        }
        return Ok(());
    }

    let val = val.unwrap();

    // 类型检查
    if !check_type(val, def.typ) {
        return Err(WorkflowError::ValidationError(format!(
            "Field '{}' expected type {:?}",
            def.name, def.typ
        )));
    }

    // 规则检查
    for rule in &def.rules {
        match rule.rule_type.as_str() {
            "required" => {} // 已经在前面处理过
            "length" => {
                // 检查字符串长度
                if let Some(s) = val.as_str() {
                    if let Some(len) = rule.length {
                        if s.len() != len {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' length must be {}",
                                    def.name, len
                                )),
                            ));
                        }
                    }
                }
            }
            "max" => {
                // 检查最大值 (数值大小 / 字符串长度 / 数组长度)
                if let Some(max) = rule.max {
                    if let Some(n) = val.as_f64() {
                        if n > max {
                            return Err(WorkflowError::ValidationError(
                                rule.message
                                    .clone()
                                    .unwrap_or(format!("Field '{}' must be <= {}", def.name, max)),
                            ));
                        }
                    } else if let Some(s) = val.as_str() {
                        if s.len() as f64 > max {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' length must be <= {}",
                                    def.name, max
                                )),
                            ));
                        }
                    } else if let Some(arr) = val.as_array() {
                        if arr.len() as f64 > max {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' size must be <= {}",
                                    def.name, max
                                )),
                            ));
                        }
                    }
                }
            }
            "min" => {
                // 检查最小值 (数值大小 / 字符串长度 / 数组长度)
                if let Some(min) = rule.min {
                    if let Some(n) = val.as_f64() {
                        if n < min {
                            return Err(WorkflowError::ValidationError(
                                rule.message
                                    .clone()
                                    .unwrap_or(format!("Field '{}' must be >= {}", def.name, min)),
                            ));
                        }
                    } else if let Some(s) = val.as_str() {
                        if (s.len() as f64) < min {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' length must be >= {}",
                                    def.name, min
                                )),
                            ));
                        }
                    } else if let Some(arr) = val.as_array() {
                        if (arr.len() as f64) < min {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' size must be >= {}",
                                    def.name, min
                                )),
                            ));
                        }
                    }
                }
            }
            "enum" => {
                // 检查枚举值
                if let Some(ref options) = rule.enum_values {
                    if !options.contains(val) {
                        return Err(WorkflowError::ValidationError(
                            rule.message.clone().unwrap_or(format!(
                                "Field '{}' must be one of {:?}",
                                def.name, options
                            )),
                        ));
                    }
                }
            }
            "pattern" => {
                // 正则表达式匹配
                if let Some(ref pat) = rule.pattern {
                    if let Some(s) = val.as_str() {
                        let re = Regex::new(pat).map_err(|e| {
                            WorkflowError::RuntimeError(format!("Invalid regex: {}", e))
                        })?;
                        if !re.is_match(s) {
                            return Err(WorkflowError::ValidationError(
                                rule.message
                                    .clone()
                                    .unwrap_or(format!("Field '{}' format invalid", def.name)),
                            ));
                        }
                    }
                }
            }
            "email" => {
                // 邮箱格式检查
                if let Some(s) = val.as_str() {
                    let email_regex =
                        Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
                    if !email_regex.is_match(s) {
                        return Err(WorkflowError::ValidationError(
                            rule.message
                                .clone()
                                .unwrap_or(format!("Field '{}' must be a valid email", def.name)),
                        ));
                    }
                }
            }
            "size" => {
                // 数组固定大小检查
                if let Some(size) = rule.size {
                    if let Some(arr) = val.as_array() {
                        if arr.len() != size {
                            return Err(WorkflowError::ValidationError(
                                rule.message.clone().unwrap_or(format!(
                                    "Field '{}' array size must be {}",
                                    def.name, size
                                )),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// 检查值类型是否匹配
fn check_type(val: &Value, typ: InputType) -> bool {
    match typ {
        InputType::STRING => val.is_string(),
        InputType::INTEGER => val.is_i64(),
        InputType::LONG => val.is_i64(),
        InputType::DECIMAL => val.is_f64(),
        InputType::BOOLEAN => val.is_boolean(),
        InputType::OBJECT => val.is_object(),
        // 文件类型暂作为对象或字符串处理
        InputType::FILE_IMAGE
        | InputType::FILE_VIDEO
        | InputType::FILE_AUDIO
        | InputType::FILE_DOCUMENT => val.is_object() || val.is_string(),
        InputType::ARRAY => val.is_array(),
        // 数组元素类型检查
        InputType::ARRAY_STRING => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_string())),
        InputType::ARRAY_INTEGER => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_i64())),
        InputType::ARRAY_LONG => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_i64())),
        InputType::ARRAY_DECIMAL => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_f64())),
        InputType::ARRAY_BOOLEAN => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_boolean())),
        InputType::ARRAY_OBJECT => val
            .as_array()
            .map_or(false, |arr| arr.iter().all(|v| v.is_object())),
        InputType::ARRAY_FILE_IMAGE
        | InputType::ARRAY_FILE_VIDEO
        | InputType::ARRAY_FILE_AUDIO
        | InputType::ARRAY_FILE_DOCUMENT => val.as_array().map_or(false, |arr| {
            arr.iter().all(|v| v.is_object() || v.is_string())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::context::{FlowContext, NodeContext};
    use crate::models::event_bus::EventBus;
    use crate::models::workflow::Node;
    use serde_json::json;
    use std::sync::Arc;

    async fn create_ctx(node_data: Value, payload: Value) -> NodeContext {
        let flow_ctx = FlowContext::new().with_payload(payload);
        let node_data = Arc::new(node_data);
        NodeContext {
            instance_id: "test-instance".to_string(),
            node: Node {
                id: "start".to_string(),
                parent_id: None,
                node_type: "start".to_string(),
                data: node_data.clone(),
                retry_policy: None,
            },
            flow_context: Arc::new(flow_ctx),
            event_bus: EventBus::new(10),
            resolved_data: node_data,
            next_nodes: Arc::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn test_start_node_validation_success() {
        let node_data = json!({
            "input": [
                {
                    "name": "age",
                    "type": "INTEGER",
                    "rules": [
                        { "type": "required" },
                        { "type": "min", "min": 18.0 }
                    ]
                },
                {
                    "name": "email",
                    "type": "STRING",
                    "rules": [
                        { "type": "email" }
                    ]
                }
            ]
        });

        let payload = json!({
            "age": 20,
            "email": "test@example.com"
        });

        let ctx = create_ctx(node_data, payload).await;
        let executor = StartNode;
        let result = executor.execute(ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_start_node_validation_fail_required() {
        let node_data = json!({
            "input": [
                {
                    "name": "age",
                    "type": "INTEGER",
                    "rules": [
                        { "type": "required" }
                    ]
                }
            ]
        });

        let payload = json!({});

        let ctx = create_ctx(node_data, payload).await;
        let executor = StartNode;
        let result = executor.execute(ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::ValidationError(msg) => assert!(msg.contains("required")),
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_start_node_validation_fail_type() {
        let node_data = json!({
            "input": [
                {
                    "name": "age",
                    "type": "INTEGER",
                    "rules": []
                }
            ]
        });

        let payload = json!({ "age": "20" }); // String instead of Int

        let ctx = create_ctx(node_data, payload).await;
        let executor = StartNode;
        let result = executor.execute(ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::ValidationError(msg) => assert!(msg.contains("expected type")),
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_start_node_validation_fail_rule() {
        let node_data = json!({
            "input": [
                {
                    "name": "age",
                    "type": "INTEGER",
                    "rules": [
                        { "type": "min", "min": 18.0, "message": "Too young" }
                    ]
                }
            ]
        });

        let payload = json!({ "age": 10 });

        let ctx = create_ctx(node_data, payload).await;
        let executor = StartNode;
        let result = executor.execute(ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::ValidationError(msg) => assert_eq!(msg, "Too young"),
            _ => panic!("Expected ValidationError"),
        }
    }
}
