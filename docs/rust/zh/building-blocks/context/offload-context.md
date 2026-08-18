---
title: "卸载上下文"
description: "显式持久化大型上下文与工具结果，并按需恢复"
---

<Note>
**Rust 实现状态**：Workspace 已提供 `WorkspaceBase::offload_context` 和 `WorkspaceBase::offload_tool_result`。它们是应用显式调用的持久化 API；当前 ReAct 自动压缩不会自动调用 offload，也不会自动把返回路径放回模型上下文。
</Note>

当消息或工具结果太大，不适合长期占用模型窗口，但又不能丢失原文时，可以把内容写入 Workspace，再在精简后的上下文中保留路径和说明。卸载解决的是“把数据保存到哪里”，压缩或裁剪解决的是“从模型输入移除什么”。

## 适用场景

适合显式卸载的内容包括：

- 很长的检索结果、日志、构建输出或网页正文；
- 后续可能核对，但当前步骤只需要摘要的历史消息；
- 包含图片、PDF、音频等 base64 数据的消息；
- 需要跨多个模型调用保留原始工具结果的任务。

以下情况通常不需要卸载：

- 内容很短，仍在预算范围内；
- 数据本身可以安全地重新生成；
- Workspace 生命周期结束后不需要恢复该数据。

卸载不会让模型自动知道文件内容。应用需要在上下文中保留返回路径，并在需要时通过受授权的读取工具取回内容，或由应用读取、反序列化后重新插入消息。

## 创建并初始化 Workspace

`LocalWorkspace` 是文件系统实现。应用应先完成初始化，并把 `WorkspaceBase` trait 引入作用域：

```rust
use agent_scope_workspace::{
    LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase, WorkspaceError,
};

async fn create_workspace(workdir: String) -> Result<LocalWorkspace, WorkspaceError> {
    let mut workspace = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });

    workspace.initialize().await?;
    Ok(workspace)
}
```

`initialize()` 是 Workspace 生命周期的必要步骤，尤其是后续需要通过 backend 或内置读取工具恢复数据时。当前 `LocalWorkspace` 的 offload 方法直接使用其受 containment 约束的 backend，并会按需创建 session/data 目录；即便如此，应用仍应先初始化 Workspace，使整个 Workspace 的生命周期和工具状态一致。文件系统问题通过 `WorkspaceError::BackendError` 等 variant 返回，消息序列化或 base64 解码问题返回 `WorkspaceError::OffloadError`。

## 卸载消息

`offload_context` 接收 session ID 和消息切片，返回写入文件的路径：

```rust
use agent_scope_message::Msg;
use agent_scope_workspace::{WorkspaceBase, WorkspaceError};

async fn persist_old_messages(
    workspace: &impl WorkspaceBase,
    session_id: &str,
    messages: &[Msg],
) -> Result<String, WorkspaceError> {
    workspace.offload_context(session_id, messages).await
}
```

对于 `LocalWorkspace`，数据流是：

1. 清理 `session_id`，使其成为安全的单一路径组件；
2. 将每条 `Msg` 序列化为一行 JSON；
3. 追加写入 `sessions/<session>/context.jsonl`；
4. 返回该 `context.jsonl` 路径。

同一 session 多次调用会追加内容，不会自动去重。因此，调用方应记录已经卸载的消息范围，避免重复写入。

### Base64 数据

消息中的 base64 `DataBlock` 会被解码并写入 Workspace 的 `data/` 目录：

- 文件名由 base64 文本的 SHA-256 hash 和 MIME 扩展名组成；
- 相同数据复用同一路径，不会重复写入；
- 写入 `context.jsonl` 的消息副本会把 data source 替换为 `file://...` URL；
- 调用方传入的原始 `Msg` 不会被原地修改。

无效 base64 会返回 `WorkspaceError::OffloadError`。

## 卸载工具结果

`offload_tool_result` 把一个 `ToolResultBlock` 转成可读文本文件：

```rust
use agent_scope_message::ToolResultBlock;
use agent_scope_workspace::{WorkspaceBase, WorkspaceError};

async fn persist_tool_result(
    workspace: &impl WorkspaceBase,
    session_id: &str,
    result: &ToolResultBlock,
) -> Result<String, WorkspaceError> {
    workspace.offload_tool_result(session_id, result).await
}
```

`LocalWorkspace` 默认写入：

```text
sessions/<session>/tool_result-<id>.txt
```

若同名文件已存在，系统使用 `-(1)`、`-(2)` 等后缀创建新文件，不覆盖旧结果。文本输出直接写入文件；结果中的 base64 数据块写入 `data/`，文件正文保存对应的 `file://...` 引用。

## 与裁剪组合

下面是推荐的数据流，而不是 ReAct 的自动行为：

1. 选择即将从模型窗口移除的旧消息或大型工具结果；
2. 显式调用 offload，并保存成功返回的路径；
3. 在上下文中写入简短摘要、路径和恢复说明；
4. 确认路径可恢复后，再调用 `trim_context` 或执行应用自己的裁剪；
5. 后续确实需要细节时，读取文件并把必要片段重新加入模型输入。

::: warning
不要先删除内存中的唯一副本，再尝试卸载。应先等待 offload 成功，随后才裁剪；发生错误时保留原消息并决定重试、改用其他存储或中止操作。
:::

当前自动压缩只移除旧消息并写入占位摘要。它不知道使用哪个 Workspace、session ID 或恢复策略，因此不会自动调用上述 API。若应用需要“压缩前卸载”，必须在自己的编排层显式实现。

恢复由应用按需执行：读取 `context.jsonl` 时逐行解析 `Msg`，并注意文件可能包含多次追加的批次；工具结果文件则是供人和模型读取的文本表示，不是 `ToolResultBlock` 的无损 JSON 序列化。若必须重建原始结构，应额外保存结构化数据。

模型能否读取返回路径还取决于 `CapabilityScope`、Workspace 工具注册和路径授权。仅把路径写进 Hint 或摘要，不会绕过这些权限边界。

## 错误处理

显式处理以下错误类别：

| 错误 | 常见原因 | 建议处理 |
|------|----------|----------|
| `NotInitialized` | 某些 Workspace 操作或其他实现要求先完成初始化 | 初始化后重试；当前 `LocalWorkspace` 的两种 offload 方法本身不以该状态为前置检查 |
| `BackendError` | 创建目录、读取或写入失败 | 保留原数据，检查 backend 和权限 |
| `OffloadError` | 消息序列化或 base64 解码失败 | 定位异常内容，不要裁剪原消息 |
| `PathTraversal` | backend 拒绝越界路径 | 不要放宽限制，修正路径来源 |

session ID 和工具结果 ID 会被清理为安全组件，但调用方仍应使用稳定、可审计的标识。不要依赖清理后的具体 hash 格式作为业务协议。

## 能力边界

| 能力 | 当前行为 |
|------|----------|
| ReAct 自动压缩 | 按 token 阈值移除旧消息，生成占位摘要 |
| Workspace offload | 由调用方显式持久化消息或工具结果，返回路径 |
| 自动“先卸载再压缩” | **尚未集成** |
| 自动恢复并重新注入 | **尚未集成** |

需要控制模型窗口时参见[压缩上下文](compress-context)；需要限制 SubAgent 对卸载文件的访问时参见[Context 概述](overview)。
