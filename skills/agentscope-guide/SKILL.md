---
name: agentscope-guide
description: 讲解 AgentScope Rust 项目(agent_scope_* crates)如何使用,并指导如何基于该框架实现一个 Agent。当用户询问"这个项目怎么用"、"如何用 AgentScope 构建/实现/集成一个 Agent"、"agent_scope 依赖怎么配置"、"ReActAgent / FunctionTool / 工具 / 记忆 / RAG / 事件 / 流式怎么用"、"从零写一个 Agent"时触发。
---

# AgentScope Rust 使用指南

一份面向开发者的 AgentScope Rust 使用指南。目标是回答两个问题:

1. **这个项目是什么、怎么用**(crate 划分、依赖如何引入、示例怎么跑)
2. **如何用这个项目实现一个自己的 Agent**(模型 → 工具 → 组装 → 对话的完整步骤)

阅读本文前无需熟悉 Python 版 AgentScope;本文只描述 Rust 侧的现状。

> **详细 API 参考**:本文是引导性文档,包含完整实现步骤。每个模块的逐字段 API、全部用法模式、错误对照表见同目录下的 **`references/` 文档**,按需查阅(见文末 [参考文档索引](#-参考文档索引))。

---

## 1. 项目概览

AgentScope Rust 是一个用 Rust 重构的 Agent 开发框架,以**多 crate workspace** 形式组织。核心能力位于 `crates/agent_scope_*`,每个 crate 是独立 package,可单独依赖。

| crate | 职责 |
|-------|------|
| `agent_scope_agent` | **Agent 编排层**:`Agent` trait、`ReActAgent`(reasoning→acting 循环)、`Middleware`、`MemoryMiddleware`、权限、`Planner`、`SubAgent` |
| `agent_scope_model` | **模型抽象**:`ChatModel` trait、`ChatResponse`、`StreamAccumulator`、`ModelCard` |
| `agent_scope_dashscope` | **Provider**:`DashScopeChatModel` / `DashScopeEmbeddingModel`(Qwen,OpenAI 兼容端点) |
| `agent_scope_message` | **消息与内容块**:`Msg`、`ContentBlock`、`user_msg`/`assistant_msg`/`system_msg` 工厂 |
| `agent_scope_event` | **事件类型**:`AgentEvent`(33 变体)流式事件定义 |
| `agent_scope_tool` | **工具系统**:`Tool` trait、`FunctionTool`、`ToolKit`、Skill 集成 |
| `agent_scope_memory` | **记忆系统**:`Memory` trait、`FileMemory`、`TurbovecMemory`、`MemoryConfig`、`MemoryEntry` |
| `agent_scope_rag` | **RAG**:`Parser`、`Chunker`、`KnowledgeBase`、`TurbovecVectorStore`、`RAGMiddleware` |
| `agent_scope_workspace` | **工作空间**:`LocalWorkspace`(隔离文件系统 + 内置工具)、Skill 管理 |
| `agent_scope_state` | **会话状态**:`AgentState`、`ReplyContext`、`SessionStore` |
| `agent_scope_sandbox` | **沙箱**(对应 microsandbox) |
| `agent_scope_embedding` / `agent_scope_types` / `agent_scope_utils` | 基础支撑 |

> **重要**:workspace 根 package `agentscope` 只是示例壳,不提供 facade 库。使用时按需依赖具体的 `agent_scope_*` crate。

🔗 详细参考:[`references/overview.md`](references/overview.md)(crate 地图与依赖 DAG)

---

## 2. 添加依赖(Cargo.toml)

### 2.1 当前:通过 GitHub 引入(尚未发布 crates.io)

项目源码托管在 **https://github.com/NingNing0111/agentscope-rust**。使用 git 依赖引入所需 crate,不要使用 `path = "../..."` 这样的相对路径(不适用于外部用户):

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_dashscope = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_tool = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_message = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_event = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }

tokio = { version = "1", features = ["full"] }
futures = "0.3"
async-trait = "0.1"
schemars = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

> 建议给 git 依赖固定一个 `rev` 或 `tag`,保证可复现构建。发布后直接用 crates.io 版本即可(见 2.2)。

### 2.2 未来:发布到 crates.io 后

项目后续会发布到 crates.io(当前版本 `0.1.0`)。发布后改为版本号即可:

```toml
[dependencies]
agent_scope_agent = "0.1"
agent_scope_dashscope = "0.1"
agent_scope_tool = "0.1"
agent_scope_message = "0.1"
agent_scope_event = "0.1"
```

🔗 详细参考:[`references/dependencies.md`](references/dependencies.md)(三种引入方式对比 + 按能力选依赖 + 常见坑)

---

## 3. 配置模型凭据

- 使用 DashScope(Qwen)需要 API Key(以 `sk-` 开头,阿里云百炼平台申请)。
- **crate 不读取环境变量**,凭据由你的应用显式传入模型构造函数。
- 常见做法:入口用 `dotenv` 加载 `.env`,再经 clap 参数或环境变量读入,最后传给 `DashScopeChatModel::new(api_key, model_name)`。

```rust
let api_key = std::env::var("API_KEY")?; // 或从 clap --api-key 读取
let model = std::sync::Arc::new(
    agent_scope_dashscope::DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true)
);
```

常用模型名:`qwen-plus`、`qwen-turbo`、`qwen-max`。`with_stream(true)` 让模型默认以流式返回(thinking 内容也会以流式 `ThinkingBlock` 返回)。

🔗 详细参考:[`references/model.md`](references/model.md)(`DashScopeChatModel` 全部字段、`DashScopeParameters`、thinking 模式、自定义 Provider)

---

## 4. 实现你的第一个 Agent

完整代码见仓库 `examples/pi-rust`(端到端 coding Agent)。下面是最小可运行路径,共四步。

### 4.1 创建模型

```rust
use std::sync::Arc;
use agent_scope_dashscope::DashScopeChatModel;

let model = Arc::new(DashScopeChatModel::new(api_key, "qwen-plus").with_stream(true));
```

需要接入非 DashScope 服务时,实现 `agent_scope_model::ChatModel` trait 即可(参考 `agent_scope_dashscope` 的实现)。

### 4.2 注册工具

把普通 async 函数包装成 `FunctionTool`。参数类型 `T` 只需实现 `Deserialize + JsonSchema`,schema 自动推导:

```rust
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    expression: String,
}

async fn calc(input: CalcInput) -> String {
    format!("calced: {}", input.expression)
}

let mut toolkit = ToolKit::new();
toolkit.register(FunctionTool::new("calculator", "Evaluate a math expression", calc));
```

handler 返回 `String`(自动转为 `ToolResultBlock`,state=Success),或直接返回 `ToolResultBlock`(无 `Result` 形式)。输入反序列化失败或 handler panic 由框架转为 `ToolError`。

> **lenient 容错反序列化**:LLM 常把数字/布尔参数序列化成字符串(`"max_results": "30"`、`"timeout_secs": "60"`)。`FunctionTool` 与 `agent_scope_tool::deserialize_lenient` 已内置容错——先严格尝试,失败后才对字符串化数字/布尔做 coerce,严格输入永不改写。因此这类工具调用不会因类型不符而被整批拒绝。

> 工具需要访问共享状态(如工作目录、配置)时,用 `Arc` 共享并在闭包里 clone(参考 `references/tools.md` 的"带状态的 handler")。

### 4.3 组装 ReActAgent

```rust
use agent_scope_agent::{AgentConfig, ContextConfig, ReActAgent, ReActConfig};

let config = AgentConfig::builder()
    .name("assistant")
    .system_prompt("You are a helpful assistant. Use the calculator tool for math questions.")
    .model(model)
    .toolkit(toolkit)
    .build()?;

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),   // 循环控制:max_iters 默认 20
    ContextConfig::default(), // 上下文压缩:默认关闭
    vec![],                   // middlewares:记忆 / RAG / 自定义
)?;
```

### 4.4 发起对话

非流式 `reply()`:

```rust
use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;

let reply = agent
    .reply(Some(vec![user_msg("user", "请用 calculator 计算 15 * 27 + 3").expect("valid user message")]))
    .await?;
println!("{}", reply.get_text_content("\n").unwrap_or_default());
```

消息由工厂函数构造:`user_msg(name, text)`、`assistant_msg(name, text)`、`system_msg(name, text)`。

`reply(None)` 表示"基于已有上下文继续回复";若上下文为空会返回 `AgentError::NoContentToReply`。`observe(...)` 只追加消息、不触发模型回复。

🔗 详细参考:[`references/agent.md`](references/agent.md)(`AgentConfig`/`ReActConfig`/`ContextConfig` 全字段、`Middleware` 9 hook、权限、`Planner`、`SubAgent`)、[`references/messages.md`](references/messages.md)(`Msg`/`ContentBlock` 全字段与工厂函数)、[`references/tools.md`](references/tools.md)(`FunctionTool`/`ToolKit`/`ToolExecOutput` 完整 API)

---

## 5. 流式对话与事件

终端 UI、WebSocket/SSE、trace 记录器用 `reply_stream()`:

```rust
use futures::StreamExt;

let mut stream = agent
    .reply_stream(Some(vec![user_msg("user", "一步步计算 (2+3)*4").expect("valid user message")]))
    .await?;

while let Some(event) = stream.next().await {
    match event {
        agent_scope_event::AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
        agent_scope_event::AgentEvent::ThinkingBlockDelta(e) => eprint!("[thinking] {}", e.delta),
        agent_scope_event::AgentEvent::ToolCallStart(e) => eprintln!("tool: {}", e.tool_call_name),
        agent_scope_event::AgentEvent::ToolResultEnd(e) => eprintln!("tool result: {:?}", e.state),
        agent_scope_event::AgentEvent::ReplyEnd(e) => eprintln!("finished: {:?}", e.finished_reason),
        _ => {}
    }
}
```

应至少处理这些事件族:

- `ReplyStart` / `ReplyEnd`:一次回复的边界
- `ModelCallStart` / `ModelCallEnd`:模型调用边界与 token 用量
- `TextBlock*` / `ThinkingBlock*`:文本与推理内容
- `ToolCall*` / `ToolResult*`:工具调用与结果
- `UserInterrupt` / `ExceedMaxIters`:控制流异常

> **注意**:同一 Agent 同一时间只允许一个 `reply()`/`reply_stream()` 活跃,并发第二个会得到 `AgentError::AlreadyStreaming`。流必须消费到 `ReplyEnd` 或主动 drop。`ReActAgent::interrupt()` 可从任意线程中断当前回复(该方法定义在 `ReActAgent` 上,不属于 `Agent` trait)。

🔗 详细参考:[`references/events.md`](references/events.md)(33 个事件变体分组、事件发布顺序、End 事件完整内容、`AppendEvent` 增量构建)

---

## 6. 权限控制

默认 `PermissionMode::Default` 下,无匹配规则时工具调用**被允许**。只读场景应显式配置:

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

let mut permission = PermissionContext::new(PermissionMode::Explore);
permission.add_rule(PermissionRule::allow("calculator"));
permission.add_rule(PermissionRule::deny("shell*"));

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .permission_context(permission)
    .build()?;
```

规则优先级:`deny` → `ask` → `allow` → 模式默认值。支持精确匹配、`*` 全匹配、`prefix*` 前缀匹配。工具执行前由 `PermissionEngine` 检查。

---

## 7. 记忆增强(MemoryMiddleware)

`MemoryMiddleware` 在系统提示中追加记忆索引,并可在回复前检索相关记忆注入上下文:

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::MemoryConfig;

let workdir = ".".to_string();
let memory_dir = "memory_data".to_string();
let middleware = Arc::new(MemoryMiddleware::with_config(
    &workdir,
    &memory_dir,
    MemoryConfig::default(),
));

let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![middleware])?;
```

底层可用 `FileMemory`(Markdown + frontmatter)或 `TurbovecMemory`(向量化长期记忆)。显式读写记忆参考 `references/memory.md`(用 `MemoryEntry`/`Memory` trait)。

🔗 详细参考:[`references/memory.md`](references/memory.md)(`Memory` trait、`MemoryEntry`、`FileMemory` 磁盘格式、`MemoryConfig` 全字段、自定义 `Backend`、`TurbovecMemory`)

---

## 8. RAG 增强(RAGMiddleware)

把文档索引进 `KnowledgeBase`,由 `RAGMiddleware` 在回复前检索相关片段:

```rust
use std::sync::Arc;
use agent_scope_embedding::EmbeddingModelCard;
use agent_scope_dashscope::DashScopeEmbeddingModel;
use agent_scope_rag::{KnowledgeBase, RAGMiddleware, RAGMode, TurbovecVectorStore};

let embedding = Arc::new(DashScopeEmbeddingModel::new(
    api_key.clone(),
    EmbeddingModelCard::new("text-embedding-v3", 1024, false),
));
let vector_store = Arc::new(TurbovecVectorStore::new(4)?); // 参数为 bit_width,合法值 2/3/4
let kb = Arc::new(KnowledgeBase::new(
    "project".to_string(),
    "Project documents".to_string(),
    embedding,
    vector_store,
    "project".to_string(),
    None,
));
let middleware = Arc::new(RAGMiddleware::new(vec![kb], RAGMode::Static, 5, None));

let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![middleware])?;
```

> **注意**:`RAGMiddleware::new` 实际签名为 `(Vec<Arc<KnowledgeBase>>, RAGMode, top_k, Option<f32>)`;`RAGMode` 取 **`Static`** 或 **`Agentic`**。Agentic 模式需把 `rag.into_search_tools()` 返回的工具注册进 `ToolKit`。

🔗 详细参考:[`references/rag.md`](references/rag.md)(`Parser`/`Chunker`/`VectorStore`/`KnowledgeBase` 完整 API、文档导入流程、Agentic vs Static、多知识库)

---

## 9. 进阶:Planner / SubAgent / Workspace

- **Planner**(`agent_scope_agent::Planner`):在任意 `Agent` 之上执行确定性的多步骤任务。规划模型输出 `{"objective","steps":[...]}` JSON,`Planner` 逐步骤驱动执行 Agent,支持失败 replanning。

  ```rust
  let planner = Planner::new(Arc::new(agent), Arc::new(planner_model), PlannerConfig::default())?;
  let result = planner.run("准备发布摘要").await?;
  ```

- **SubAgent**(`agent_scope_agent::SubAgent`):父 Agent 通过 `SubAgentRegistry` 注册具名 SubAgent,用 `delegate_once()` / `delegate_many()` 委派有边界的子任务,`ContextSharingPolicy` 默认最小权限。

- **Workspace**(`agent_scope_workspace::LocalWorkspace`):给 Agent 一个隔离文件系统,内置 Read/Write/Edit/Bash/Glob/Grep 工具与 Skill 管理:

  ```rust
  use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

  let config = LocalWorkspaceConfig {
      workdir: "/tmp/my-workspace".into(),
      workspace_id: None,
      default_mcps: vec![],
      skill_paths: vec![],
      instructions: None,
  };
  let mut ws = LocalWorkspace::new(config);
  ws.initialize().await?;
  ```

🔗 详细参考:[`references/workspace.md`](references/workspace.md)(`LocalWorkspace`/`WorkspaceManager`/`Skill` 管理/MCP)、[`references/session.md`](references/session.md)(`Session`/`SessionStore`/上下文裁剪)。`LocalSandboxSession` 沙箱定义在 `agent_scope_sandbox` crate,用法见 `references/workspace.md` §9。

---

## 10. 从零实现一个完整 Agent(分步构建指南)

把前面章节串成一个完整流程。假设场景:**一个带工具、流式、记忆和 RAG 的问答 Agent**。

### 10.1 项目骨架

```text
my-agent/
├── Cargo.toml
└── src/
    └── main.rs
```

`Cargo.toml` 按第 2 节添加 git 依赖,外加:

```toml
[dependencies]
dotenv = "0.15"
anyhow = "1"   # 或 thiserror,看你偏好
```

### 10.2 main.rs 骨架

```rust
use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, MemoryMiddleware, PermissionContext, PermissionRule,
    ReActAgent, ReActConfig,
};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_memory::MemoryConfig;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let api_key = std::env::var("API_KEY").expect("set API_KEY");

    // 1. 模型
    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    // 2. 工具(见第 4.2 节)
    let mut toolkit = /* 注册你的 FunctionTool */;

    // 3. 权限
    let mut permission = PermissionContext::default();
    permission.add_rule(PermissionRule::allow("calculator"));

    // 4. 记忆 middleware(可选)
    let middleware = Arc::new(MemoryMiddleware::with_config(
        ".", "memory_data", MemoryConfig::default(),
    ));

    // 5. 组装 Agent
    let config = AgentConfig::builder()
        .name("my_agent")
        .system_prompt("You are a helpful assistant.")
        .model(model)
        .toolkit(toolkit)
        .permission_context(permission)
        .build()?;
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![middleware],  // 不需要记忆就传 vec![]
    )?;

    // 6. 流式对话
    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "你好,用工具算 15*27+3").expect("valid user message")]))
        .await?;
    while let Some(event) = stream.next().await {
        match event {
            agent_scope_event::AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
            agent_scope_event::AgentEvent::ReplyEnd(e) => {
                eprintln!("\nfinished: {:?}", e.finished_reason);
            }
            _ => {}
        }
    }
    Ok(())
}
```

### 10.3 逐阶段扩展

| 想加的能力 | 改动位置 | 参考 |
|-----------|---------|------|
| 更多工具 | `ToolKit` 里 `register` | `references/tools.md` |
| thinking 模式 | `model.parameters.enable_thinking = true` | `references/model.md` |
| RAG 文档知识库 | 构造 `RAGMiddleware` 加入 middlewares | `references/rag.md` |
| 自定义 middleware(日志/审计) | 实现 `Middleware` trait | `references/agent.md` §6 |
| 会话持久化 | `agent_scope_state` 的 `SessionStore` | `references/session.md` |
| 多步骤规划 | `Planner::new(agent, planner_model, config)` | `references/agent.md` §8 |
| 子任务委派 | `SubAgentRegistry` + `delegate_once()` | `references/agent.md` §9 |
| 文件/Shell 操作 | `LocalWorkspace` 或 `LocalSandboxSession` | `references/workspace.md` |
| 事件完整渲染 | match 全部事件族,参考 `examples/pi-rust/src/render.rs` | `references/events.md` |

---

## 11. 运行仓库自带示例

```bash
# 从仓库根目录克隆后
cargo run -p pi-rust -- --help
cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"

# 交互式 coding Agent(需 API_KEY)
export API_KEY="sk-your-key"
cargo run -p pi-rust -- --workdir .pi-rust --cwd . --model qwen-plus --mode coding --show-events
```

`examples/pi-rust` 演示了完整的真实用法:模型、ReAct 循环、四个编码工具(Read/Write/Edit/Bash)、Coding workflow、Skills 加载、权限、MemoryMiddleware、RAGMiddleware、会话持久化。它是"真实 Agent 长什么样"的最佳参考。

> **Skill 实时扫描**:pi-rust 的 `Skill` 工具不再使用启动时快照,而是每次调用实时扫描 `workspace/skills` 目录(`LocalSkillLoader::list_skills_blocking`)。运行中把新 skill 目录复制进 `workspace/skills` 立即生效,无需重启;`PermissionRule::allow("Skill")` 始终开启以支持运行期动态装入。

---

## 12. 常见问题

| 现象 | 原因与解决 |
|------|-----------|
| 编译报错找不到 crate | git 依赖未固定 rev,或 `cargo` 缓存问题;先 `cargo update`,必要时清 `~/.cargo/git/checkouts` |
| API 返回 401 / invalid api key | 凭据无效;crate 不读环境变量,确保显式传入有效 `sk-` Key |
| 提示 `Agent is busy (already streaming)` | 上一次流式回复未结束又发起新回复;等当前 stream 结束或 drop |
| `reply(None)` 报 `NoContentToReply` | 上下文为空;先传用户消息或调用 `observe(Some(...))` |
| 工具输入解析失败 | 模型生成的 JSON 与参数类型不符;工具 schema 描述写清楚 |
| 模型不调用工具 | 确认 `AgentConfig` 传了 `toolkit`,且 system prompt 里引导使用工具 |
| 事件里没有 thinking 内容 | thinking 需要 `enable_thinking = true` 且模型默认流式(`with_stream(true)`) |
| 想禁止某些工具被调用 | 加 `PermissionRule::deny("tool_name*")`,或用 `PermissionMode::Explore` 白名单 |

---

## 13. 参考文档索引

本 skill 的 `references/` 目录包含各模块的详细 API 参考(逐字段签名、全部用法、错误对照、兼容性):

| 文档 | 内容 |
|------|------|
| [`references/overview.md`](references/overview.md) | 项目概览、crate 地图、依赖 DAG、何时用哪个 crate |
| [`references/dependencies.md`](references/dependencies.md) | Cargo.toml 三种引入方式、按能力选依赖、常见坑 |
| [`references/messages.md`](references/messages.md) | `Msg`/`ContentBlock` 全字段、工厂函数、角色校验、序列化协议 |
| [`references/model.md`](references/model.md) | `ChatModel` trait、`ModelCallResult`、`StreamAccumulator`、`DashScopeChatModel`/`Parameters`、thinking |
| [`references/events.md`](references/events.md) | `AgentEvent` 33 变体分组、发布顺序、End 累积内容、`AppendEvent`、取消 |
| [`references/tools.md`](references/tools.md) | `Tool` trait、`FunctionTool`、`ToolKit`、`ToolExecOutput`、生命周期、Skill 集成 |
| [`references/agent.md`](references/agent.md) | `Agent` trait、`ReActAgent`、`AgentConfig`/`ReActConfig`/`ContextConfig`、`Middleware`、权限、`Planner`、`SubAgent` |
| [`references/memory.md`](references/memory.md) | `Memory` trait、`FileMemory`、`MemoryConfig`、`MemoryMiddleware`、`Backend`、`TurbovecMemory` |
| [`references/rag.md`](references/rag.md) | `Parser`/`Chunker`/`VectorStore`/`KnowledgeBase`/`RAGMiddleware`、文档导入、Agentic vs Static |
| [`references/workspace.md`](references/workspace.md) | `LocalWorkspace`/`WorkspaceManager`/Skill 管理/MCP;`LocalSandboxSession` 沙箱(定义于 `agent_scope_sandbox`) |
| [`references/session.md`](references/session.md) | `Session`/`SessionImpl`/`AgentState`/`SessionStore`/上下文裁剪 |

---

## 14. 进一步阅读

- 仓库内文档:`docs/zh/getting-started.md`(30 分钟上手)、`docs/zh/modules/*.md`(消息、事件、模型、工具、Agent、记忆、RAG、Workspace、Sandbox 等分模块详解)
- 完整参考实现:`examples/pi-rust`(crate 用法、CLI、REPL、会话)
- 版本说明:`CHANGELOG.md`;Python 版参考:`migration.md`
- 相关外部项目:TurboVec(向量存储)、microsandbox(沙箱后端)
