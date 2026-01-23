use sonyflake::Sonyflake;
use std::sync::OnceLock;
use crate::models::error::WorkflowError;

pub struct Id;

impl Id {
    /// 懒加载初始化，这里的 expect 是安全的，因为如果配置正确，它只在启动第一次执行
    fn instance() -> &'static Sonyflake {
        static INSTANCE: OnceLock<Sonyflake> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Sonyflake::builder()
                .finalize()
                .expect("Critical: Failed to init IdGenerator at startup")
        })
    }

    /// 生产级：返回 Result，不直接 panic
    pub fn next_id() -> Result<u64, WorkflowError> {
        Self::instance().next_id().map_err(WorkflowError::from)
    }
}
