//! upflow - 异步工作流引擎
//!
//! # 概述
//! upflow 是一个基于 Rust 的异步工作流引擎，旨在提供高效、可靠的工作流执行能力。
//! 它使用有向无环图 (DAG) 来表示工作流，并支持复杂的节点类型和执行策略。
//!
//! # 核心模块
//! - `core`: 定义核心枚举和常量
//! - `engine`: 包含工作流执行引擎、图构建器和运行时环境
//! - `models`: 定义数据模型，包括节点、边、事件等
//! - `nodes`: 实现各种类型的节点（起始节点、分支节点、子流程节点等）
//! - `utils`: 提供辅助功能，如ID生成、路径解析等
//!
//! # 主要功能
//! - 基于 DAG 的工作流定义
//! - 多种节点类型支持（起始、结束、决策、子流程等）
//! - 异步执行模型
//! - 工作流验证机制
//! - 事件总线系统

pub(crate) mod core;
pub(crate) mod engine;
pub(crate) mod models;
pub(crate) mod nodes;
pub mod utils;

pub mod prelude;
