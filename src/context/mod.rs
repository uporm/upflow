use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::model::error::WorkflowError;
use dashmap::DashMap;
use regex::Regex;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct FlowContext {
    pub payload: Value,
    pub data: Arc<DashMap<String, Value>>,
    pub sys_vars: Arc<HashMap<String, Value>>,
}

impl FlowContext {
    pub fn new() -> Self {
        Self {
            payload: Value::Null,
            data: Arc::new(DashMap::new()),
            sys_vars: Arc::new(HashMap::new()),
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_sys_vars(mut self, sys_vars: HashMap<String, Value>) -> Self {
        self.sys_vars = Arc::new(sys_vars);
        self
    }

    pub fn set_output(&self, node_id: &str, output: Value) {
        self.data.insert(node_id.to_string(), output);
    }

    pub fn get_output(&self, node_id: &str) -> Option<Value> {
        self.data.get(node_id).map(|v| v.value().clone())
    }

    pub fn resolve_value(&self, value: &Value) -> Result<Value, WorkflowError> {
        match value {
            Value::String(s) => self.resolve_string(s),
            Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(self.resolve_value(item)?);
                }
                Ok(Value::Array(out))
            }
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, v) in map {
                    out.insert(k.clone(), self.resolve_value(v)?);
                }
                Ok(Value::Object(out))
            }
            _ => Ok(value.clone()),
        }
    }

    fn resolve_string(&self, raw: &str) -> Result<Value, WorkflowError> {
        let re = placeholder_regex()?;
        let mut matches = re.captures_iter(raw).collect::<Vec<_>>();
        if matches.is_empty() {
            return Ok(Value::String(raw.to_string()));
        }
        if matches.len() == 1 {
            if let Some(m) = matches.pop() {
                let whole = m.get(0).map(|m| m.as_str()).unwrap_or("");
                let key = m.get(1).map(|m| m.as_str()).unwrap_or("");
                if whole == raw.trim() {
                    return self.resolve_variable(key.trim());
                }
            }
        }
        let mut output = raw.to_string();
        for cap in re.captures_iter(raw) {
            let key = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let resolved = self.resolve_variable(key.trim())?;
            let rep = match resolved {
                Value::String(s) => s,
                other => serde_json::to_string(&other)
                    .map_err(|e| WorkflowError::RuntimeError(e.to_string()))?,
            };
            output = output.replace(cap.get(0).unwrap().as_str(), &rep);
        }
        Ok(Value::String(output))
    }

    fn resolve_variable(&self, expr: &str) -> Result<Value, WorkflowError> {
        let mut parts = expr.splitn(2, '.').collect::<Vec<_>>();
        if parts.is_empty() {
            return Ok(Value::Null);
        }
        let head = parts.remove(0);
        if head == "sys" {
            let tail = parts.get(0).copied().unwrap_or("");
            if tail.is_empty() {
                return Ok(json!({}));
            }
            if let Some(value) = self.sys_vars.get(tail) {
                return Ok(value.clone());
            }
            return Ok(Value::Null);
        }
        if head == "input" {
            let tail = parts.get(0).copied().unwrap_or("");
            return Ok(self.extract_path(&self.payload, tail));
        }
        let base = self.get_output(head).unwrap_or(Value::Null);
        let tail = parts.get(0).copied().unwrap_or("");
        Ok(self.extract_path(&base, tail))
    }

    fn extract_path(&self, value: &Value, path: &str) -> Value {
        if path.is_empty() {
            return value.clone();
        }
        let mut current = value;
        for seg in path.split('.') {
            match current {
                Value::Object(map) => {
                    if let Some(v) = map.get(seg) {
                        current = v;
                    } else {
                        return Value::Null;
                    }
                }
                Value::Array(arr) => {
                    let idx = seg.parse::<usize>().ok();
                    if let Some(i) = idx {
                        if let Some(v) = arr.get(i) {
                            current = v;
                        } else {
                            return Value::Null;
                        }
                    } else {
                        return Value::Null;
                    }
                }
                _ => return Value::Null,
            }
        }
        current.clone()
    }
}

fn placeholder_regex() -> Result<&'static Regex, WorkflowError> {
    static REGEX: OnceLock<Result<Regex, String>> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"\{\{\s*([^}]+)\s*}}").map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| WorkflowError::RuntimeError(format!("regex error: {}", e)))
}
