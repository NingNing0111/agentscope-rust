# Feature Specification: Pi Coding Agent (Rust)

**Feature Branch**: `023-pi-coding-agent`

**Created**: 2026-08-02

**Status**: Draft

**Input**: User description: "使用rust重构的agentscope，实现pi这个项目，不需要完整复刻，但是功能要一样。examples/pi-rust，原pi项目是examples/pi-rust/pi-ts, Rust实现的pi不应该依赖pi-ts的任何内容，pi-ts仅供参考，设计Agent。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Interactive Coding Assistant (Priority: P1)

用户启动 pi-rust CLI，进入交互式 REPL 界面，输入自然语言编码任务（如"读取 main.rs 并解释其功能"），Agent 通过工具调用读取文件、分析代码，并流式返回结构化回答。用户可以持续进行多轮对话，Agent 在上下文中保持对之前交互的记忆。

**Why this priority**: 这是 pi 编码 Agent 的核心价值——一个可以理解代码、使用工具并持续对话的交互式助手。没有这个场景，产品不具备任何用户价值。

**Independent Test**: 启动 CLI，输入"读取 src/main.rs 文件内容"，验证 Agent 调用了读取工具并返回了文件内容摘要。可以进行多轮对话验证上下文保持。

**Acceptance Scenarios**:

1. **Given** pi-rust CLI 已启动且当前目录存在 `src/main.rs`，**When** 用户输入"请读取 src/main.rs 并告诉我它的主要功能"，**Then** Agent 调用 Read 工具读取文件，流式输出分析结果，且结果正确描述了文件的主要功能。
2. **Given** 上一轮对话已完成（Agent 已回答了关于 main.rs 的问题），**When** 用户追问"这个文件里有哪些函数？"，**Then** Agent 基于上下文理解"这个文件"指的是 main.rs，并正确列出函数名。
3. **Given** 用户正在与 Agent 对话，**When** 用户输入空行，**Then** Agent 不产生任何响应，等待下一次有效输入。

---

### User Story 2 - Code Editing and File Operations (Priority: P1)

用户请求 Agent 修改代码文件。Agent 调用 Write 或 Edit 工具对指定文件进行精确修改。Agent 在修改前能够读取文件确认当前内容，修改后告知用户变更摘要。

**Why this priority**: 编码 Agent 的核心区别在于能实际修改代码，而不仅仅是回答问题。这是与纯聊天机器人相比的关键差异。

**Independent Test**: 让 Agent 创建一个小文件，然后修改它，验证文件内容正确。

**Acceptance Scenarios**:

1. **Given** 工作目录可写，**When** 用户输入"创建一个 hello.txt 文件，内容为 'Hello, World!'"，**Then** Agent 调用 Write 工具创建文件，且文件内容完全匹配。
2. **Given** hello.txt 已存在且内容为 "Hello, World!"，**When** 用户输入"把 hello.txt 中的 World 改成 Rust"，**Then** Agent 调用 Edit 工具精确替换文本，文件内容变为 "Hello, Rust!"。
3. **Given** 目标文件不存在，**When** 用户请求编辑不存在的文件，**Then** Agent 报告文件不存在错误，并提供合理的下一步建议。

---

### User Story 3 - Shell Command Execution (Priority: P2)

用户请求 Agent 执行 shell 命令（如 `cargo build`、`ls`、`git status`）。Agent 在执行前对潜在风险命令请求用户确认，执行后返回命令输出。

**Why this priority**: Bash 执行是编码工作流的关键环节——构建、测试、版本控制都依赖 shell。但相比文件操作，它涉及更多安全考量。

**Independent Test**: 让 Agent 执行 `pwd` 或 `ls`，验证返回正确的工作目录或文件列表。

**Acceptance Scenarios**:

1. **Given** pi-rust 在某个工作目录下运行，**When** 用户输入"执行 pwd 告诉我当前目录"，**Then** Agent 调用 Bash 工具执行 `pwd`，返回正确的工作目录路径。
2. **Given** 用户请求执行一个可能修改文件的命令（如 `rm some_file.txt`），**When** Agent 识别到该命令具有潜在风险，**Then** Agent 在执行前请求用户确认。
3. **Given** 用户请求执行的命令输出较长（如 `find . -name "*.rs"`），**When** 命令执行完成，**Then** Agent 对输出进行合理截断或摘要，避免淹没对话上下文。

---

### User Story 4 - Session Persistence and Recovery (Priority: P2)

用户在一个会话中进行的对话可以被保存，并在下次启动时恢复。用户退出后重新启动 pi-rust，可以选择恢复之前的会话继续工作。

**Why this priority**: 编码任务通常跨多个 session——开发者今天没完成的任务明天继续。会话持久化使 Agent 不是"失忆"的。

**Independent Test**: 开始一个会话进行对话，退出，重新启动选择恢复，验证上下文已恢复。

**Acceptance Scenarios**:

1. **Given** 用户在一个会话中与 Agent 进行了多轮对话，**When** 用户执行 `/exit` 退出，**Then** 会话数据被持久化保存。
2. **Given** 之前存在已保存的会话，**When** 用户重新启动 pi-rust 并选择恢复该会话，**Then** Agent 加载之前的对话历史，用户可以从上次离开的地方继续。
3. **Given** 没有可恢复的会话，**When** 用户启动 pi-rust，**Then** Agent 创建一个全新的会话开始交互。

---

### User Story 5 - Multi-Provider LLM Support (Priority: P3)

用户可以通过配置选择不同的 LLM 提供商（DashScope、OpenAI 兼容接口等）。默认使用 DashScope，但用户可以切换。

**Why this priority**: 多提供商支持增加了灵活性，但初始版本可以先聚焦一个提供商确保核心功能稳定。

**Independent Test**: 使用不同提供商配置启动 CLI，验证 Agent 能正常调用对应 API。

**Acceptance Scenarios**:

1. **Given** 用户设置了 API_KEY 环境变量，**When** 启动 pi-rust 不指定 provider 参数，**Then** Agent 默认使用 DashScope 作为模型提供商。
2. **Given** 用户通过命令行参数指定了不同的模型名称，**When** 启动 pi-rust，**Then** Agent 使用指定的模型进行推理。

---

### Edge Cases

- 当用户输入极长文本（超过模型上下文窗口）时，Agent 应合理截断或提示用户。
- 当 LLM API 调用超时或返回错误时，Agent 应优雅降级并提示用户重试，而非崩溃。
- 当 Bash 命令执行超时（如死循环）时，Agent 应在可配置的时间后终止命令并返回部分输出。
- 当文件被外部进程同时修改时，Edit 工具应检测冲突（如文件内容与读取时不一致）。
- 当用户快速连续发送多条消息时，Agent 应按序处理，不应丢失或混淆消息。
- 当工作目录权限不足（如只读文件系统）时，Write/Edit 工具应返回明确的权限错误。
- 当 Agent 多轮对话的上下文累计超过模型上下文窗口时，应触发上下文压缩（compaction）机制。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 提供交互式命令行界面（REPL），接受用户自然语言输入并流式返回 Agent 响应。
- **FR-002**: 系统 MUST 提供 Read 工具，能够读取指定路径的文件内容并返回。
- **FR-003**: 系统 MUST 提供 Write 工具，能够创建或覆盖指定路径的文件。
- **FR-004**: 系统 MUST 提供 Edit 工具，能够对指定文件的特定文本进行精确替换。
- **FR-005**: 系统 MUST 提供 Bash 工具，能够在受限的工作目录内执行 shell 命令并返回输出。
- **FR-006**: 系统 MUST 基于 ReActAgent（推理-行动循环）模式运行，Agent 自主决定何时调用工具、何时给出最终回答。
- **FR-007**: 系统 MUST 支持流式输出，用户能实时看到 Agent 的文本生成过程和工具调用状态。
- **FR-008**: 系统 MUST 维护多轮对话上下文，Agent 能够引用之前轮次的交互内容。
- **FR-009**: 系统 MUST 通过 MemoryMiddleware 支持长期记忆的写入和检索。
- **FR-010**: 系统 MUST 支持会话数据的持久化保存和恢复。
- **FR-011**: 系统 MUST 支持通过命令行参数配置模型名称、API Key、工作目录等关键参数。
- **FR-012**: 系统 MUST 处理 LLM API 调用失败（超时、网络错误、认证失败），向用户返回有意义的错误信息而非崩溃。
- **FR-013**: 系统 MUST 在 Bash 工具中对潜在危险命令（如 `rm -rf`、文件系统修改命令等）请求用户确认。
- **FR-014**: 系统 SHOULD 在对话上下文接近模型限制时触发上下文压缩（compaction），保留关键信息。
- **FR-015**: 系统 SHOULD 通过 LocalWorkspace 管理工作目录内的文件操作，确保文件操作可追踪。
- **FR-016**: 系统 SHOULD 支持 RAG（检索增强生成），允许用户索引项目文档并在对话中检索相关上下文。

### Key Entities

- **Agent Session**: 代表一次完整的交互会话，包含对话历史、上下文状态、会话 ID 和创建时间。可被持久化和恢复。
- **Tool**: Agent 可调用的能力单元。每种工具（Read/Write/Edit/Bash）有独立的参数 schema、执行逻辑和结果格式。
- **Conversation Turn**: 一轮用户-Agent 交互，包含用户消息、Agent 的思考/工具调用序列、以及最终响应文本。
- **Context**: Agent 当前的完整上下文，包括系统提示词、对话历史、注入的记忆和 RAG 检索结果。
- **Permission Context**: 定义工具的执行权限，控制哪些工具可以不经确认直接执行，哪些需要用户批准。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 用户从启动 CLI 到完成第一轮有效对话（Agent 正确响应一个编码问题）的时间不超过 10 秒（不含 LLM API 延迟）。
- **SC-002**: Agent 在执行编码任务（读文件→分析→修改）时，工具调用成功率不低于 95%（工具调用格式正确且参数有效）。
- **SC-003**: 用户可以在 3 轮以内的对话中完成一个简单的编码任务（如"读取文件 → 修改一行代码 → 保存"）。
- **SC-004**: 连续 20 轮对话内，Agent 的上下文保持准确率不低于 90%（能正确引用前 5 轮内的关键信息）。
- **SC-005**: 会话恢复后，Agent 能够正确引用之前会话中的关键对话内容。
- **SC-006**: 当 API 调用失败时，Agent 在 2 秒内向用户报告错误并提供重试建议，而非静默卡死。

## Assumptions

- 用户已安装 Rust 工具链并能编译运行 Rust 项目。
- 用户拥有有效的 DashScope API Key（或其他兼容提供商的 API Key）。
- 底层 agentscope-rust 框架的 ReActAgent、Tool、Memory、Workspace、RAG 等模块已实现并稳定可用。
- pi-rust 作为 examples 下的示例项目存在，复用而非重新实现 agentscope-rust 框架的核心能力。
- pi-rust 不依赖 pi-ts（TypeScript 项目）的任何代码或运行时，pi-ts 仅作为功能参考。
- 初始版本聚焦单 Agent 交互模式，多 Agent 协作（Team/SubAgent）作为后续扩展。
- 初始版本以 DashScope 为主要 LLM 提供商，OpenAI 兼容接口作为可配置选项。
- Bash 工具默认在工作目录内执行，危险命令需要确认。
- 上下文压缩（compaction）策略使用 agentscope-rust 框架已有的摘要机制。
