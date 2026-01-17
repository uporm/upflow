use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

pub struct WorkflowContext {
    pub run_id: String,
    pub variables: Arc<DashMap<String, Value>>,
    pub depth: u32,
}

impl WorkflowContext {
    pub fn new(depth: u32) -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            variables: Arc::new(DashMap::new()),
            depth,
        }
    }

    pub fn get_var(&self, path: &str) -> Option<Value> {
        let mut parts = path.split('.');
        let first_part = parts.next()?;
        
        // 获取 root 的引用，此时持有 DashMap 的读锁
        let root_ref = self.variables.get(first_part)?;
        let mut current_val = root_ref.value();
        
        // 遍历路径，只移动引用
        for part in parts {
            current_val = current_val.get(part)?;
        }
        
        // 最后只 clone 一次
        Some(current_val.clone())
    }

    pub fn set_var(&self, key: String, val: Value) {
        self.variables.insert(key, val);
    }
}