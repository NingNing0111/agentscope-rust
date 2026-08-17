# Quickstart: 任务工具输出质量优化 验证指南

**Feature**: 033-task-tools-optimization | **Date**: 2026-08-17

本文档定义端到端验证场景，证明特性按 spec 工作。输出文本协议见 `contracts/task-tools-output.md`；设计决策见 `research.md`、`data-model.md`。

## 前置条件

```bash
cd /Users/pgthinker/StudyCode/GithubProject/agentscope-rust
cargo --version   # stable toolchain
```

无需真实 LLM——核心场景使用 Scripted/Mock Model（宪法第六条）。

## 场景 1：连续工具结果独立成行（对应 US1 / FR-001~002 / SC-001）

Scripted Model 在一轮中连续产生 3 次 `TaskCreate`（同一 reply 多个工具调用），驱动 ReAct 循环。

```bash
rtk cargo test -p agent_scope_agent task_tools
```

**预期**:
- 每个任务工具结果文本以 `\n` 结尾（断言：`text.ends_with('\n')`）
- 连续 3 个 TaskCreate 结果的文本各自独立成行，不拼接：
  ```
  Task (id=1) created successfully: A
  Task (id=2) created successfully: B
  Task (id=3) created successfully: C
  ```
- 事件流（`ToolResultTextDelta`）中，前一结果结束与后一结果开始之间有换行分隔

## 场景 2：展示层对非任务工具结果补换行（对应 FR-002 / SC-001 #3）

任一未以 `\n` 结尾的非任务工具（如输出单行的工具）完整结果，经流式/批处理层发射后以 `\n` 结尾。

```bash
rtk cargo test -p agent_scope_agent
```

**预期**: 完整工具结果文本若原样不以 `\n` 结尾，事件 delta 与上下文存储文本均以 `\n` 补全；已以 `\n` 结尾的不重复追加（幂等）。

## 场景 3：TaskUpdate 报告实际变更值（对应 US2 / FR-003 / SC-002）

直接调用 TaskUpdate 工具验证四种输出（不经模型，用工具单元测试）：

```bash
rtk cargo test -p agent_scope_agent task_tools
```

**预期**（逐字对照 `contracts/task-tools-output.md` §4）:
- 仅更新状态：`Updated task (id=2): status=in_progress`
- 同时更新状态与依赖：`Updated task (id=1): status=in_progress; add_blocked_by=[4]`
- 更新后为 completed：`Updated task (id=3): status=completed` + 空行 + `Task completed. Call TaskList now to find your next available task or see if your work unblocked others.`
- 无实际变更：`No updates were made to the task (id=1). Make sure you provided at least one field to update and the values are correct.`
- 删除：`Task (id=2) has been deleted.`
- 不存在：`TaskNotFoundError: The task (id=99) does not exist.`
- 全部以 `\n` 结尾

## 场景 4：TaskGet 描述截断（对应 US3 / FR-004 / SC-003）

构造描述长度分别 >200 / ==200 / <200 / 空 的 4 个任务，调用 TaskGet。

```bash
rtk cargo test -p agent_scope_agent task_tools
```

**预期**（对照 `contracts/task-tools-output.md` §3）:
- `len > 200`：`Description: {前 200 字符}… (truncated, {len} chars total)`
- `len == 200`：完整描述（不截断）
- `len < 200`：完整描述
- 空描述：`Description: `（空行）
- 任务不存在：`Task not found`（Error state）

## 场景 5：示例端到端与完整验收（对应 US4 / FR-009 / SC-001）

有真实 API key 时运行示例，目视检查输出：

```bash
DASHSCOPE_API_KEY=xxx rtk cargo run -p plan-react-agent -- --prompt "请规划并执行：1) 阅读本仓库根目录的 README.md；2) 列出其中提到的三个 crate；3) 汇总成一段话。"
```

**预期**:
- `[tool] TaskCreate/List/Get/Update registered: true`
- 同一轮多个工具结果各自独立成行，不拼接
- 工具结果与模型推理文本之间以换行分隔，不粘连
- TaskUpdate 结果显示实际变更值（如 `status=in_progress`）

完整验收命令（宪法第十七条定义 of done）：

```bash
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --check
```

**预期**: 全 workspace 测试通过（含迁移后的任务工具断言）、clippy 零警告、fmt 通过。

## 场景 6：兼容性登记（对应 FR-006~007 / SC-005）

```bash
python3 -c "
import json
m = json.load(open('specs/001-compatibility-baseline/capability-matrix.json'))
for e in m['entries']:
    if e.get('capability_id','').startswith('tool-task-'):
        print(e['capability_id'], '| notes contains deviation:', 'Feature 033' in e.get('notes',''))
"
```

**预期**: `tool-task-create/list/get/update` 四条 `notes` 均含 Feature 033 输出文本偏差登记（仿 ResetTools 命名偏差做法）。

## 回归基线

```bash
rtk cargo test --workspace   # 既有 ~950+ 测试全部通过（任务工具断言已迁移到新协议）
```

完成定义参照宪法第十七条 checklist（单元测试、无静默降级、文档更新、示例可编译、clippy/fmt 通过、兼容矩阵已更新、无未登记 UnsupportedFeature）。
