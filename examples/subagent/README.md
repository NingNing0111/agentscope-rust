# `subagent` 示例

**工具驱动**的多智能体委托示例：把 SubAgent 封装成两个内置工具，由主 Agent 在 ReAct 循环里**自主创建并委托**子智能体完成任务。所有子智能体均为真实 DashScope 模型调用，不包含任何模拟实现。

## 核心思路

| 工具 | 作用 |
|------|------|
| `SubAgentCreate` | 主 Agent 调用它**创建并注册**一个真实 `ReActAgent` 子智能体（名称 / 职责 / 角色指令） |
| `SubAgentDelegate` | 主 Agent 调用它把任务**委托**给已创建的子智能体，并把结果回填给主 Agent |

两个工具与主 Agent 共享同一个 `SubAgentRegistry`（`Arc<tokio::sync::RwLock>`）。主 Agent 持有这两个工具后，由模型自主决定：拆解任务 → `SubAgentCreate` 建研究员 / 编码 / 复核子智能体 → `SubAgentDelegate` 逐个派发 → 汇总结果。

## 运行

```bash
cargo run -p subagent                            # 默认模型 qwen-plus + 内置示例任务
cargo run -p subagent -- --model qwen-max        # 指定模型
cargo run -p subagent -- --task "你的自定义任务"  # 自定义任务
```

凭据从项目根目录 `.env` 读取（`DASHSCOPE_API_KEY`），也支持环境变量：

```bash
export DASHSCOPE_API_KEY="sk-your-key"
```

缺凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 运行输出

主循环用 `reply_stream` 消费事件流，按事件类型打印核心事件，可以看到主 Agent 自主决策的完整过程：

1. 工具注册确认（`[toolkit]` 行）与输入的任务；
2. **流式事件**（按类型带标记打印）：
   - `[reply start]` / `[reply end]` — 一次回复的生命周期；
   - `[model call]` — 每次模型调用开始；
   - `[tool call]` … `[tool end]` — 主 Agent 调用 `SubAgentCreate` / `SubAgentDelegate`；
   - `[tool result]` … `[tool result end]` — 子智能体的产出以工具结果增量流回；
   - 思考增量（暗色）与主 Agent 汇总文本（明文）实时逐字输出；
3. 主 Agent 实际创建的子智能体清单（从共享注册表列出，验证「由主 Agent 自己创建」）。

## 关键点

- 工具用 `agent_scope_tool::FunctionTool` 封装，输入结构体加 `schemars::JsonSchema` + `serde::Deserialize`，schema 自动推导；
- `SubAgentCreate` 是幂等的：同名子智能体已注册则直接提示，不会重复构建；
- `SubAgentDelegate` 内部走 `delegate_once`；若目标未创建会返回「未找到」类结果文本，主 Agent 会据此先创建再重试；
- 子智能体（`build_subagent`）不继承主 Agent 的 toolkit——工具归属完全由各自的 `AgentConfig` 决定。
