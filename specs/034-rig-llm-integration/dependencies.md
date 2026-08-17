# Feature 034 依赖治理记录（T003 / FR-011）

**日期**: 2026-08-17
**目标**: 引入 `rig` 作为 LLM provider 框架，核实依赖图、license、维护/安全状况并记录。

## 版本锁定

| 依赖 | 版本 | 来源 | license | repository | 备注 |
|------|------|------|---------|------------|------|
| rig | 0.41.0 | crates.io 发布版 | MIT | github.com/0xPlaygrounds/rig | 0.42.0 仅存在于 rig 仓库主分支，未发布；锁定 crates.io 0.41.0 |

> ⚠️ **版本勘误**：research.md 初版记为 rig 0.42.0，源自 rig 仓库主分支源码（workspace version 0.42.0）。
> crates.io 实际最新发布为 **0.41.0**，故锁定 `rig = "0.41"`。Explore agent 已据此将 API 调研对象改为 crates.io 0.41.0。

## 依赖图（反向）

```
rig v0.41.0
└── agent_scope_rig v0.1.0
    └── agentscope v0.1.0
```

- rig 仅经 `agent_scope_rig` 引入，无循环依赖。
- rig facade 默认 features = `rig-core/default + agent + derive + rustls`；openai/anthropic/deepseek provider 内置于 rig-core，无额外 feature 门控。

## 维护/安全评估

- **维护活跃度**: rig 由 0xPlaygrounds 维护，crates.io 迭代频繁（0.36→0.41 跨度短），社区采用度高。
- **license**: MIT，与仓库 Apache-2.0 兼容。
- **unsafe**: rig-core 本身非 `#![deny(unsafe_code)]`（其内部依赖如 reqwest 含 unsafe），但 `agent_scope_rig` 外层 `#![deny(unsafe_code)]` 保证本 crate 无 unsafe 代码（宪法第九条）。

## 实际编译树

`cargo check -p agent_scope_rig`（2026-08-17）确认默认 features 实际编译进树的 crate：

```
rig v0.41.0
├── rig-core v0.41.0        （openai/anthropic/deepseek provider 内置于 rig-core，无 feature 门控）
├── rig-agent v0.41.0       （agent + derive feature）
├── reqwest v0.13.4         （rustls 后端）
│   ├── rustls v0.23.42
│   ├── aws-lc-rs v1.18.0
│   └── hyper v1.11.0 / hyper-util v0.1.20
└── tokio-rustls v0.26.4 / rustls-platform-verifier v0.7.0
```

- **rmcp 未编译**：rig facade 的 `rmcp ^2.2.0` 声明仅在 MCP feature 下生效，默认 features 不引入；项目 `agent_scope_mcp` 用 rmcp 3.x 互不冲突（Feature 027 已验证）。
- 无重复/循环依赖；rig 仅经 `agent_scope_rig` 引入（T035 复核 `cargo tree -i rig`）。

## 后续复核

- T035 阶段复核 `cargo tree -i rig` 无重复/循环依赖。
- rig 升级治理：升级前需回归 agent_scope_rig 映射层 + 示例，登记 capability-matrix。
