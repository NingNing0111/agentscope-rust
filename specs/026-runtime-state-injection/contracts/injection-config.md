# Contracts: 注入配置（InjectionConfig）

**Feature**: 026-runtime-state-injection | **Date**: 2026-08-04
**上游基准**: Python AgentScope `9d1026fa` `agent/_config.py::InjectionConfig`

## 公开表面

`agent_scope_agent::config::InjectionConfig` — 新增的公开配置类型，通过 `AgentConfigBuilder::injection_config(...)` 注入 `AgentConfig`。字段命名、默认值与 Python `InjectionConfig` 对齐。

| 字段 | Rust 类型 | Python 默认 | 说明 |
|------|-----------|-------------|------|
| `inject_runtime_state` | `bool` | `true` | 总开关 |
| `timezone` | `String` | `"UTC"` | IANA 时区名 |
| `time_format` | `String` | `"%Y-%m-%dT%H:%M:%S"` | 时间格式 |
| `time_interval` | `f64` | `0.5` | 最小注入间隔（小时） |
| `context_buffer_ratio` | `f64` | `0.2` | 上下文用量缓冲比例 |
| `template` | `String` | 见 [默认模板](#默认模板) | 注入包装模板 |
| `injection_source` | `String` | `{"label": "System", "sublabel": "Runtime State"}` | 注入来源标识 |
| `task_tool_names` | `Vec<String>` | `["TaskCreate","TaskGet","TaskList","TaskUpdate"]` | 任务感知检测的工具名 |
| `extra_fields` | `HashMap<String, String>` | `{}` | 附加字段 |
| `emit_hint_event` | `bool` | `true` | 是否发射 HintBlockEvent |

## 默认模板

```text
<system-reminder>Treat the following as the ground truth at this point of the conversation. Anything stated earlier is outdated, and a later reminder, if any, supersedes this one:
{runtime_state}
</system-reminder>
```

## 构造与校验

### 构造方式

```rust
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .injection_config(InjectionConfig::default())   // 显式
    // 或省略 → 默认 InjectionConfig
    .build()?;
```

未显式设置时使用 `InjectionConfig::default()`（`inject_runtime_state=true` 等 Python 默认值）。

### 校验规则（构建时执行）

`AgentConfig::build` 调用 `InjectionConfig::validate()`，任何违规返回 `AgentError::InvalidConfig { field, message }`：

| 字段 | 规则 | 错误信息要点 |
|------|------|--------------|
| `template` | 必须包含 `{runtime_state}` | `injection template must contain {runtime_state}` |
| `time_format` | 必须可往返完整时间戳 | `time_format must round-trip a full timestamp` |
| `time_interval` | `>= 0` | `time_interval must be >= 0` |
| `context_buffer_ratio` | `0 <= r <= 1` | `context_buffer_ratio must be in [0, 1]` |
| `context_buffer_ratio` | `< ContextConfig.trigger_ratio` | `context_buffer_ratio must be smaller than trigger_ratio` |
| `timezone` | **不校验**（运行时回退 UTC） | — |

### 兼容性说明

- 无效时区名不拒绝配置，运行时回退 UTC（对齐 Python `test_invalid_timezone_falls_back_to_utc`）。
- `time_format` 往返校验为 Rust 侧增强（Python 未校验），用于避免静默丢弃字段（FR-007）。
- 字段默认值全部对齐 Python `InjectionConfig`，支撑差分/黄金快照测试。

## 交互

| 配置 | 影响 |
|------|------|
| `inject_runtime_state = false` | 三维均不注入，不发事件，行为退化为纯推理-行动循环；不影响任务工具注册（与 `task_tools_enabled` 独立） |
| `task_tools_enabled = false` | 不注册任务工具；任务维度注入亦被抑制（024 兼容基线），时间/上下文维度不受影响 |
| `emit_hint_event = false` | 注入发生但事件流不发射 `HintBlockEvent` |
| `extra_fields` 非空 | 附着于每次注入；单独设置不触发注入 |
| `task_tool_names` 自定义 | 任务感知检测改用自定义工具名列表 |
