---
title: "工作空间管理"
description: "多租户工作空间的部署形态管理"
---

<Note>
**Rust 实现状态**: 部分支持（兼容等级 L2）。已支持：**库级多租户工作空间管理**（`WorkspaceManager`：按 ID 创建/获取、TTL 清理，`agent_scope_workspace::manager`）。尚未实现：服务化/远程部署形态的 `per_agent` / `per_session` / `per_user` 隔离策略与资源配额。兼容基线为 AgentScope Python v2.0.5。
</Note>

# 工作空间管理（部分支持）

工作空间管理的服务化形态是多租户部署（`per_agent` / `per_session` / `per_user` 隔离策略）。AgentScope Rust 在**库级**提供 `WorkspaceManager` 实现多租户隔离与生命周期管理，但非部署/远程形态。

## 库级 WorkspaceManager

| 能力 | Rust 状态 |
|------|-----------|
| 按 ID 创建 / 获取工作空间 | ✅ `WorkspaceManager::new` + `create` / `get` |
| 空闲 TTL 清理 | ✅ TTL 参数（`Duration`） |
| per_agent / per_session / per_user 隔离策略 | ❌ 未实现 |
| 远程 / 部署形态管理 | ❌ 未实现 |
| 资源配额模型 | ❌ 未实现（见 [manage-resources](../building-blocks/workspace/manage-resources)） |

库级用法见 [管理资源](../building-blocks/workspace/manage-resources)。

## 缺失范围

- 无 per_agent / per_session / per_user 隔离策略：`WorkspaceManager` 以自定义配置函数决定工作空间创建，未内置会话维度隔离。
- 无远程 / 部署形态：`LocalWorkspace` 为本地文件系统后端，Docker / E2B / K8s 为「计划中」。

## 替代能力

- 库级 `WorkspaceManager`（多租户 + TTL）见 [管理资源](../building-blocks/workspace/manage-resources)。
- 可在 Rust 侧自建 HTTP 服务包装 `WorkspaceManager` 暴露为部署形态。

## 相关

- 库级基础能力见对应的 building-blocks 页面。
- 逐页状态对照见 `mirror-map.md`。
