# Contract: Agent Demo CLI

**Date**: 2026-08-01 | **Feature**: [../spec.md](../spec.md)

本文定义 `cargo run --example agent_demo -- ...` 的用户可见 CLI 契约。实现阶段可调整内部模块结构，但不得改变这里定义的核心用户体验，除非同步更新 plan/design artifacts。

## Command

```bash
cargo run --example agent_demo -- [OPTIONS]
```

## Required Cargo registration

根 `Cargo.toml` 必须包含：

```toml
[[example]]
name = "agent_demo"
path = "examples/agent-demo/main.rs"
```

## Options

| Option | Values / Type | Default | Description |
|--------|----------------|---------|-------------|
| `--mode <mode>` | `deterministic` \| `live` | `deterministic` | 选择离线确定性路径或可选 live provider 路径 |
| `--api-key <key>` | string | `API_KEY` env in live mode | DashScope live mode 凭据；输出必须脱敏 |
| `--model <name>` | string | project example default | live mode 模型名 |
| `--workspace-dir <path>` | path | demo temp/target dir | demo artifact 输出目录 |
| `--trace-json <path>` | path | none | 写出结构化 trace JSON |
| `--show-coverage` | bool | false | 在运行结束后打印 capability coverage table |
| `--fail-tool` | bool | false | deterministic mode 下触发受控 tool failure |
| `--cancel-after-step <step-id>` | string | none | deterministic mode 下在指定 step 后触发 cancellation |
| `--verbose` | bool | false | 输出额外非敏感诊断 |
| `--help` | bool | n/a | 打印帮助 |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Demo completed successfully |
| 1 | Demo ran but failed due to model/tool/scenario error |
| 2 | Configuration or usage error (e.g., live mode missing API key) |
| 130 | Demo cancelled intentionally |

## Output contract

### Successful deterministic run

必须包含以下 human-readable sections（标题可本地化/美化，但语义需保留）：

1. Demo title + run mode
2. Preflight summary（workspace path、trace output path if any；不得打印 secret）
3. Scenario timeline（按 step 顺序）
4. Tool invocation summary（工具名、参数安全摘要、结果摘要）
5. Session/memory continuity evidence（至少两轮）
6. Final answer/summary
7. Coverage table（当 `--show-coverage` 或 README 推荐验证命令中启用）

### Live mode missing configuration

当 `--mode live` 且无 `--api-key`/`API_KEY` 时，必须在调用 provider 前失败，并输出：

- 缺少的配置名：`API_KEY` 或 `--api-key`
- 如何修复：示例命令或 `.env` 提示
- 明确说明 deterministic mode 可无需凭据运行
- 不输出任何凭据值

### Tool failure injection

当 `--fail-tool` 启用时：

- Tool call event 必须出现
- Tool error 必须以稳定 category/code 输出
- Demo 可以以非零退出，或展示 Agent 如何处理 tool error；无论哪种，README/quickstart 必须记录期望

### Cancellation injection

当 `--cancel-after-step <step-id>` 启用时：

- Trace 必须包含 cancellation event
- Exit code 推荐为 130
- 已完成步骤的 trace 不得丢失

## Secret handling

- `--api-key` 和 `API_KEY` 原值不得出现在 terminal output、trace JSON、workspace artifact 或 error message 中。
- 可使用完全省略或 mask 格式（如 `sk-***abcd`）。
- `--verbose` 不得绕过 secret masking。

## Stability expectations

- `--mode deterministic --show-coverage` 的高层 section、capability ids、trace event kinds 应保持稳定，以便 maintainer regression。
- Live mode 的自然语言内容不作为唯一验收依据；只验证流程、错误处理和脱敏行为。
