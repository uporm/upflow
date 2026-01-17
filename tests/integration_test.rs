use std::sync::Arc;
use uflow::{WorkflowEngine, WorkflowContext};
use serde_json::json;
use tokio::fs;

#[tokio::test]
async fn test_unknown_node() {
    // 读取 flow.json
    let content = fs::read_to_string("tests/resources/flow.json").await.expect("Failed to read flow.json");
    
    // 加载
    let engine = WorkflowEngine::global();
    engine.load("test_flow", &content).expect("Failed to load flow");
    
    // 运行，期望失败，因为包含 loop, assign 等未知节点
    let result = engine.run("test_flow").await;
    assert!(result.is_err());
    
    if let Err(e) = result {
        println!("Expected error: {}", e);
        // 验证错误信息包含 "Unknown node type"
        assert!(e.to_string().contains("Unknown node type"));
    }
}

#[tokio::test]
async fn test_simple_flow() {
    let engine = WorkflowEngine::global();
    
    // 构造一个简单的 flow: start -> decision -> end
    // Decision 检查 input.age > 18
    let flow_json = json!({
        "nodes": [
            {
                "id": "start1",
                "type": "start",
                "data": { "input": [] }
            },
            {
                "id": "decision1",
                "type": "decision",
                "data": {
                    "cases": [
                        {
                            "id": "adult",
                            "opr": "and",
                            "conditions": [
                                { "varId": "input.age", "opr": "gt", "value": 18 }
                            ]
                        }
                    ]
                }
            },
            {
                "id": "end1",
                "type": "end",
                "data": {}
            }
        ],
        "edges": [
            { "source": "start1", "target": "decision1", "sourceHandle": null },
            { "source": "decision1", "target": "end1", "sourceHandle": "adult" }
        ]
    });
    
    engine.load("simple_flow", &flow_json.to_string()).expect("Failed to load simple flow");
    
    let ctx = Arc::new(WorkflowContext::new(0));
    // 设置变量
    ctx.set_var("input".to_string(), json!({ "age": 20 }));
    
    // 运行
    let result = engine.run_with_context("simple_flow", ctx.clone()).await;
    assert!(result.is_ok());
    
    // 验证是否到达 end1？
    // 目前没有机制检查执行路径，但如果没有 panic 且 result is ok，说明流程跑通了。
    // 如果 age < 18，流程应该在这里停止（死路），或者如果所有分支都不匹配，decision 默认输出 default handle (None)。
    // 在这个定义里，如果 decision 不匹配，next_handle 是 None。
    // edge 要求 sourceHandle="adult"。如果不匹配，decision output handle != "adult"，
    // Executor 会 decrement target (end1) 的 degree 吗？
    // 让我们看 executor 代码：
    // if Some(h) != output.next_handle.as_ref() { decrement... continue }
    // 是的，即使不匹配，也会 decrement，但这是为了让后续节点知道“我这一路不通”。
    // 等等，decrement degree 会导致后续节点入度减 1。如果入度减为 0，节点就会被执行。
    // 这意味着：不匹配的分支也会触发后续节点？
    // 不，这取决于设计。
    // 通常 Decision 分支如果不走，后续节点不应该执行。
    // 但如果后续节点只有 1 个入度，decrement 后变成 0，就会执行。
    // 这意味着 uflow 的设计是：只要前置节点执行完了（不管结果如何），后续节点都会执行？
    // 不，看代码：
    // if Some(h) != output.next_handle.as_ref() { ... continue; }
    // 这里的 continue 跳过了什么？
    // 跳过了后续的逻辑？不，这里是循环 edges。
    // 这里的逻辑是：
    // 1. 如果 edge 指定了 sourceHandle (比如 "adult")
    // 2. 且 output.next_handle (实际走的路径) != edge.sourceHandle
    // 3. 那么这个 edge 是“死路”。
    // 4. 死路也要 decrement degree 吗？
    // 是的，代码里调用了 decrement_degree。
    // 如果 decrement 后 degree == 0，后续节点会被触发。
    // 这意味着：分支没选中，后续节点依然会执行！
    // 这是一个巨大的逻辑 BUG。
    // 在工作流引擎中，如果分支没选中，后续路径上的节点应该被跳过（或者标记为 skipped）。
    // 如果仅仅是减少入度，那么后续节点会误以为所有前置都完成了，然后开始执行。
    // 除非后续节点的执行逻辑里检查了“是否被跳过”。
    // 但 NodeExecutor 只接收 ctx 和 data。
    // 这是一个非常核心的调度算法问题。
    // 通常做法是：死路传播 (Dead Path Elimination)。
    // 如果一条路是死的，沿着这条路的所有节点都应该被标记为死（skipped），并且继续传播死标记，直到遇到汇聚点（Join）。
    // 这里的代码：
    // Self::decrement_degree(&edge.target, &mut degree_lock, &tx_clone).await;
    // 只是简单地减入度。
    
    // 如果 uflow 的设计是“所有节点都会执行，只是状态不同”，那没问题。
    // 但看代码，节点执行完全由 `rx.recv()` 驱动。
    // 只要入度为 0，就丢进 `tx`，然后被执行。
    // 所以，目前的代码会导致：Decision 的所有分支（包括未选中的）后续节点都会被执行。
    // 这显然是错误的。
    
    // 这是一个非常严重的 Bug。
    // 修复这个问题比较复杂，需要引入“死路标记”。
    // 简单的修复：如果分支不匹配，不 decrement degree？
    // 如果不 decrement，后续节点的入度永远 > 0，永远不会执行。这对于“死路”是对的。
    // 但是，如果后续节点是一个汇聚点（Join），它等待多个前置。
    // 如果其中一个前置是死路，它不 decrement，那么汇聚点永远等不到入度为 0，永远 hang 住。
    // 所以必须 decrement。
    // 但是 decrement 导致入度为 0 时，如果不应该执行，该怎么办？
    // 应该有一种机制通知后续节点：“你的前置完成了，但是是死路”。
    // 或者，在 decrement 时传递一个 flag。
    // 这是一个典型的 Dead Path Elimination 问题。
    
    // 鉴于这是一个“优化代码”的任务，修复核心调度算法可能超出范围，或者风险太大。
    // 但作为高级助手，我应该指出并尝试修复，或者至少在测试中暴露它。
    // 让我先写测试，验证这个行为。
    // 如果我设置 age = 10 (不满足 > 18)，end1 应该不执行。
    // 如果 end1 执行了，说明 bug 存在。
    // 怎么知道 end1 执行了？
    // 可以检查 end1 是否修改了变量。
    // 修改 end1 的 data，让它写入一个变量。
    // 但 EndNode 的实现是空的。
    // 我可以用 assign 节点测试。但我没实现 assign。
    // 我可以用 subflow 节点测试，或者修改 EndNode 让它写个标记。
    // 或者用 Decision 节点做后续。
    
    // 让我们先把之前的集成测试跑通，看看是否有其他问题。
}
