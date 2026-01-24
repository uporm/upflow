use std::collections::HashMap;
use std::sync::OnceLock;
use regex::Regex;
use serde_json::{json, Value};
use crate::models::error::WorkflowError;
use crate::models::context::FlowContext;

pub fn resolve_value(ctx: &FlowContext, value: &Value) -> Result<Value, WorkflowError> {
    match value {
        Value::String(s) => resolve_string(ctx, s),
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(resolve_value(ctx, item)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(ctx, v)?);
            }
            Ok(Value::Object(out))
        }
        _ => Ok(value.clone()),
    }
}

fn resolve_string(ctx: &FlowContext, raw: &str) -> Result<Value, WorkflowError> {
    let re = placeholder_regex()?;
    let trimmed = raw.trim();
    let mut full_match_iter = re.captures_iter(trimmed);
    if let Some(cap) = full_match_iter.next()
        && full_match_iter.next().is_none()
    {
        let whole = cap.get(0).map(|m| m.as_str()).unwrap_or("");
        if whole == trimmed {
            let key = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            return resolve_variable(ctx, key.trim());
        }
    }

    let mut output = String::with_capacity(raw.len());
    let mut last = 0;
    let mut found = false;
    let mut cache: HashMap<String, String> = HashMap::new();
    for cap in re.captures_iter(raw) {
        found = true;
        let m = cap.get(0).unwrap();
        output.push_str(&raw[last..m.start()]);
        let key = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let rep = if let Some(rep) = cache.get(key) {
            rep.clone()
        } else {
            let resolved = resolve_variable(ctx, key)?;
            let rep = match resolved {
                Value::String(s) => s,
                other => serde_json::to_string(&other)
                    .map_err(|e| WorkflowError::RuntimeError(e.to_string()))?,
            };
            cache.insert(key.to_string(), rep.clone());
            rep
        };
        output.push_str(&rep);
        last = m.end();
    }
    if !found {
        return Ok(Value::String(raw.to_string()));
    }
    output.push_str(&raw[last..]);
    Ok(Value::String(output))
}

fn resolve_variable(ctx: &FlowContext, expr: &str) -> Result<Value, WorkflowError> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(Value::Null);
    }
    let mut parts = expr.splitn(2, '.');
    let head = parts.next().unwrap_or("");
    let tail = parts.next().unwrap_or("");
    match head {
        "sys" => {
            if tail.is_empty() {
                Ok(json!({}))
            } else {
                Ok(ctx.env.get(tail).cloned().unwrap_or(Value::Null))
            }
        }
        _ => {
            let base = ctx.get_result(head);
            match base {
                Some(value) => Ok(extract_path(value.as_ref(), tail)),
                None => Ok(Value::Null),
            }
        }
    }
}

fn extract_path(value: &Value, path: &str) -> Value {
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

fn placeholder_regex() -> Result<&'static Regex, WorkflowError> {
    static REGEX: OnceLock<Result<Regex, String>> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"\{\{\s*([^}]+)\s*}}").map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| WorkflowError::RuntimeError(format!("regex error: {}", e)))
}
