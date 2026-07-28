<!--
Sync Impact Report
==================
Version change: N/A (initial template) → 1.0.0
This is the initial ratification of the AgentScope Rust Constitution.
Added: All 19 principles across 7 sections, Governance section.
Removed: None (initial creation).
Templates requiring updates:
  ✅ .specify/templates/plan-template.md — "Constitution Check" section references exist, compatible as-is
  ✅ .specify/templates/spec-template.md — User scenarios + requirements align with constitution expectations
  ✅ .specify/templates/tasks-template.md — Task phases align with small-step delivery principle
  ✅ .claude/skills/speckit-*/SKILL.md — All skill files reference constitution only generically, no updates needed
No follow-up TODOs; all placeholders resolved.
-->

# AgentScope Rust 工程宪法

**项目名称**: AgentScope Rust (agent_scope)

**项目目标**: 使用 Rust 重构 AgentScope，在锁定的上游版本范围内实现高兼容实现——公开 API、数据协议、运行行为、事件顺序、错误语义、流式输出、工具调用流程和示例行为与 Python 参考实现保持可观察一致性。

**适用范围**: 本项目所有 crate、模块、测试、文档和发布产物。本宪法是本项目的最高权威文件，所有设计决策、代码审查和发布流程必须以本宪法为最终依据。

---

## 第一部分：兼容性与行为基准

### 第一条：兼容性优先

**Rule**:
所有公开能力 MUST 优先保证与 AgentScope Python 参考实现的外部可观察兼容性。此要求涵盖：

- 输入参数名称、类型及默认值
- 返回值结构及其 JSON 序列化格式
- Message 与 ContentBlock 的结构定义
- 事件类型及事件发布顺序
- 流式响应的 chunk 顺序
- Tool Call 与 Tool Result 的完整生命周期
- Agent reasoning-acting 循环行为
- Middleware 注册与执行顺序
- Memory 和 Session 状态变化的语义
- Timeout、Cancellation 和 Shutdown 的行为表现
- 错误分类及机器可读错误码
- 用户、Session 和 Tenant 之间的隔离机制

内部模块、数据结构和算法 MAY 与 Python 实现不同，但 MUST NOT 改变任何外部可观察结果。

**Rationale**:
AgentScope Rust 对用户来说不是一个新框架——它是 AgentScope 的 Rust 实现。用户基于 Python 文档和现有代码编写的逻辑在 Rust 版本上应当产生等价结果，否则迁移成本将抵消 Rust 在性能和安全性方面的收益。

**Enforcement**:
- 每个公开模块 MUST 通过差分测试（Rust vs Python 黄金快照）验证兼容性。
- Code review MUST 检查：新增公开接口是否偏离了 Python 参考实现的行为，如有偏离是否已记录为已知偏差。
- 兼容性矩阵 MUST 在每个功能发布时更新。

**Exceptions**:
- 对 Python 实现中的明确缺陷（bug），Rust 实现 MAY 采取不同行为，但 MUST 在兼容性矩阵中记录该偏差并说明原因。
- Rust 语言自身强制的行为差异（如整数溢出行为、字符串编码）MAY 豁免兼容性要求，但 MUST 记录。

---

### 第二条：锁定上游版本

**Rule**:
兼容目标 MUST 绑定到一个明确的、不可变的 AgentScope 上游版本，至少包含：

- AgentScope Python 包的 release version（如 `1.0.0`）
- Git commit hash（如 `a1b2c3d4e5f6`）
- 已知的 Python 版本
- 关键依赖的版本号

持续变化的 `main` 分支 MUST NOT 作为模糊兼容目标。

所有兼容性报告、测试数据和发布版本 MUST 记录对应的上游 commit。

升级 AgentScope 上游版本时 MUST 作为独立的 specification 执行，MUST NOT 在常规功能迭代中静默改变兼容基线。

**Rationale**:
兼容性需要可验证的参照物。如果上游版本持续变化，"兼容"无从定义。锁定版本使得每次比较都在已知基线上进行。

**Enforcement**:
- CI 中 MUST 存在一个被锁定的 Python AgentScope 环境用于差分测试。
- 每次发布时 MUST 在 CHANGELOG 中记录上游 commit。
- 任何修改兼容基线的 PR MUST 同时更新上游版本记录并重新生成所有黄金快照。

**Exceptions**:
- 对上游已修复的安全漏洞，MAY 提前在 Rust 侧修复而不等待正式的上游版本升级，但 MUST 记录。

---

### 第三条：Python AgentScope 是行为基准

**Rule**:
AgentScope Python 实现是兼容性判断的最终行为基准。

每项受支持能力 MUST 经过以下流程验证：

1. 使用相同输入运行 Python AgentScope 参考实现
2. 记录输出、事件、状态变化和所有可观察副作用
3. 使用相同输入运行 Rust 实现
4. 对两套结果进行标准化处理
5. 通过差分测试（diff test）比较两套结果

开发人员 MUST NOT 仅根据文档、类型定义或个人理解推测行为。

当文档、源码和运行结果不一致时：
- 文档用于理解设计意图
- 源码用于理解实现逻辑
- **实际运行结果**用于确定外部可观察兼容行为

**Rationale**:
文档可能过时，源码可能包含未被文档化但被用户依赖的实际行为。只有实际运行结果才能确切定义什么是"可观察行为"。

**Enforcement**:
- 每个 capability specification MUST 包含 Python 参考实现的实际运行记录。
- CI 中的差分测试 MUST 基于 Python 实际运行结果，而非文档描述。
- 当发现文档/源码/行为不一致时，MUST 在 specification 中记录三方差异及最终决策。

**Exceptions**:
- 对于 AgentScope 中尚未发布或文档化但源码中存在的实验性功能，MAY 暂不纳入兼容范围，但 MUST 明确标记。

---

## 第二部分：规格与质量门

### 第四条：先定义契约，再实现代码

**Rule**:
任何公共模块在进入实现阶段前 MUST 拥有已批准的 specification。

Specification MUST 至少定义以下内容：

- 用户场景（User Scenarios）
- 支持范围（Scope）
- 输入输出契约（Input/Output Contract）
- 生命周期（Lifecycle）
- 状态机（State Machine）
- 事件协议（Event Protocol）
- 序列化协议（Serialization Protocol）
- 并发行为（Concurrency Behavior）
- 取消行为（Cancellation Behavior）
- 超时行为（Timeout Behavior）
- 错误契约（Error Contract）
- 边界情况（Edge Cases）
- 非目标（Non-Goals）
- 兼容性测试用例（Compatibility Test Cases）
- 可量化验收条件（Quantifiable Acceptance Criteria）

没有 specification 的公共功能 MUST NOT 进入实现阶段。

**Rationale**:
先写 specification 迫使开发人员在编码前想清楚"什么是兼容行为"。没有明确契约的代码无法被验证——你不知道它在满足什么需求。在兼容性项目中，specification 是测试基准的源头。

**Enforcement**:
- PR review 中，任何触及公共 API 的变更 MUST 引用对应的 specification。
- CI MUST 验证：所有 `pub` 符号属于某个 specification 定义的能力。
- 新增模块时，specification 先于代码提交。

**Exceptions**:
- 内部（`pub(crate)` 或更低可见性）辅助模块 MAY 豁免完整 specification，但 MUST 有基本文档说明其用途。

---

### 第五条：不允许伪兼容

**Rule**:
尚未实现或无法兼容的能力 MUST 显式返回稳定的 `UnsupportedFeature` 错误。以下行为被严格禁止：

- 静默忽略不支持的参数
- 返回虚假的成功结果（如空 JSON `{}` 表示操作成功）
- 使用空实现（no-op）冒充完整功能
- 自动降级但不通知调用方
- 捕获错误后继续执行并隐藏异常
- 为了让测试通过而删除关键行为检查

所有暂不支持的能力 MUST 登记在兼容性矩阵（Compatibility Matrix）中，标注为 `unsupported` 并附错误码。

**Rationale**:
伪兼容比明确不支持更危险。调用方会误以为功能可用，在生产环境中靠假实现运行，导致难以追踪的 bug。明确报错让调用方能做决策——等待、降级或换方案。

**Enforcement**:
- CI MUST 扫描代码，禁止存在"接收参数但完全不使用"的模式（通过 lint 或自定义检查）。
- 兼容性矩阵 MUST 在每次发布前更新。
- Code review 中，任何看似"占位"的实现 MUST 被标记并要求返回 `UnsupportedFeature`。

**Exceptions**:
- 无。

---

### 第六条：测试驱动兼容性

**Rule**:
每项公共能力 MUST 包含以下测试中的至少涵盖的全部适用类型：

- Rust 单元测试
- Python 黄金快照测试（Golden Snapshot Test）
- Python 与 Rust 差分测试（Diff Test）
- 序列化往返测试（Serialization Round-Trip Test）
- Property-Based Test
- 并发和取消测试
- Provider 协议测试
- 示例迁移测试（Example Migration Test）
- 回归测试（Regression Test）

核心兼容性 MUST NOT 仅依赖真实 LLM 的自然语言输出结果来判断。

测试 MUST 优先使用以下可重复组件，以保证结果确定性：

- Mock Model（返回固定响应）
- Scripted Model（按脚本返回预设序列）
- Recorded Model（回放录制的真实响应）
- 固定 Tool（参数与返回值确定）
- 固定 Clock（可控时间源）
- 固定 ID Generator（可控标识符生成）

**Rationale**:
真实 LLM 的输出是非确定的——温度、模型更新、网络延迟都会改变结果。兼容性测试的目标是验证框架行为，而非模型行为。固定组件消除外部不确定性，保证每次运行得到相同结论。

**Enforcement**:
- 每个能力 specification MUST 列出其测试策略。
- CI 中差分测试 MUST 使用 Mock/Recorded Model。
- PR 合并前 MUST 通过所有兼容性测试。

**Exceptions**:
- 端到端示例迁移测试 MAY 使用真实 LLM 作为辅助验证，但 MUST NOT 作为唯一的兼容性判定依据。

---

### 第七条：Trace 是核心验收产物

**Rule**:
Agent 的最终文本输出不是唯一的验收结果。

测试 MUST 根据能力记录并比较完整 Trace，包括：

- Model request（请求内容）
- Model response（响应内容）
- Streaming chunks（流式分块）
- Tool calls（工具调用）
- Tool arguments（工具参数）
- Tool results（工具结果）
- Middleware hooks（中间件钩子执行）
- Agent events（Agent 事件）
- Memory writes（内存写入）
- Session state changes（会话状态变化）
- Errors（错误信息）
- Cancellation（取消标记）
- Final output（最终输出）

在比较前 MAY 标准化以下非确定字段：

- 时间戳（Timestamp）
- UUID
- Trace ID
- Request ID
- Provider ID
- 网络耗时
- Token 延迟
- Map key 的顺序
- 可接受范围内的浮点误差

以下内容 MUST NOT 被忽略或抹除：

- 事件的先后顺序
- Tool 的参数值
- Role（角色标签）
- Finish reason（完成原因）
- 错误类型
- 状态变化
- 副作用
- Cancellation 状态

**Rationale**:
Agent 框架的价值不仅在于"给出了什么答案"，更在于"在什么状态下、经过怎样的步骤、产生了哪些副作用"。只有完整 trace 才能验证框架行为兼容性。

**Enforcement**:
- 差分测试 MUST 比较结构化 trace，而非最终文本字符串。
- Trace diff 工具 MUST 支持配置哪些字段可以标准化。
- 任何新增的副作用（如新的事件类型）MUST 在 trace 规范中定义后方可加入。

**Exceptions**:
- 性能基准测试 MAY 仅关注端到端延迟和吞吐，不要求完整 trace。

---

## 第三部分：Rust 原生设计

### 第八条：Rust 原生设计

**Rule**:
Rust 实现 MUST NOT 机械复制 Python 的继承体系、动态类型和运行时反射行为。

Rust 侧实现 SHOULD 优先使用以下符合 Rust 生态的设计模式：

| 模式 | 用途 |
|------|------|
| 明确的数据类型（struct/enum） | 表达数据和有限状态 |
| `enum` | 表达有限状态和可选变体 |
| `trait` | 定义扩展接口和行为抽象 |
| `Result<T, E>` | 表达可恢复失败 |
| `Arc` | 管理共享所有权 |
| 结构化并发（Structured Concurrency） | 管理异步任务生命周期 |
| 有界 Channel（Bounded Channel） | 模块间通信与背压 |
| 显式 CancellationToken | 取消传播 |
| 明确的生命周期标注与资源所有者 | 资源安全 |

公开接口 SHOULD 优先保证易用性和稳定性，SHOULD NOT 为了追求零成本抽象而暴露过度复杂的泛型参数。

动态扩展点 SHOULD 优先使用 trait object 模式：

- `Arc<dyn ChatModel>`
- `Arc<dyn Tool>`
- `Arc<dyn Memory>`
- `Arc<dyn Middleware>`
- `Arc<dyn Workspace>`

经过性能分析（profiling）确认为热点的路径 MAY 引入更复杂的泛型和静态分发设计，但 MUST 附 profiling 结果作为依据。

**Rationale**:
Rust 和 Python 的抽象模式天然不同。机械翻译会产生反模式代码，浪费 Rust 的类型安全优势。但泛型过度也会导致编译时间膨胀和 API 复杂度上升。在实践中，trait object 在扩展点提供足够灵活性的同时保持了简洁。

**Enforcement**:
- Code review MUST 检查：是否存在不必要的 `Box<dyn Any>` 或反射模拟。
- 泛型公开 API MUST 在 specification 中说明性能依据。
- 编译时间 MUST 被 CI 监控，不应无理由增长。

**Exceptions**:
- 在与 Python 实现共享的测试工具代码中，MAY 使用更接近 Python 风格的辅助结构以降低 test fixture 维护成本。

---

### 第九条：安全 Rust 优先

**Rule**:
`unsafe` Rust MUST 默认被禁止。

只有在同时满足以下所有条件时才 MAY 使用 `unsafe`：

1. 无法通过安全 Rust 合理实现同样的功能
2. 被隔离在独立的、明确命名的模块中（如 `mod sync_unsafe`）
3. Safety Invariants MUST 以文档注释形式明确记录
4. MUST 提供专项测试（含 Miri 检测）
5. MUST 经过独立的代码审查（由非作者完成）
6. MUST NOT 向上层代码泄露不安全约束（即安全封装的上层代码 MUST 在不了解内部 unsafe 细节的情况下也是安全的）

库代码中，以下 panic 倾向的函数和宏 MUST NOT 无理由使用：

- `unwrap()`
- `expect()`
- `panic!()`
- `todo!()`
- `unimplemented!()`

测试代码（`#[cfg(test)]`）和已被证明不可失败的内部不变量（如 `NonZeroUsize::new(1).unwrap()`）MAY 例外使用上述函数，但 MUST 以注释说明安全理由。

**Rationale**:
Rust 的核心价值是内存安全保证。AgentScope 是一个并发多 agent 框架，unsafe 的漏洞可能在复杂交互中被放大。同时，滥用 unwrap 会造成不可恢复的 panic——在 long-running agent 服务中，一个错误 input 不应导致整个进程崩溃。

**Enforcement**:
- CI MUST 运行 `cargo clippy` 并设置 `#![deny(unsafe_code)]` 于非 unsafe 模块。
- CI MUST 运行 `cargo miri test` 于任何包含 unsafe 的模块。
- PR review 中，任何新增 unsafe MUST 被标记为阻塞项，直至满足全部条件。

**Exceptions**:
- FFI 边界（如调用 C 语言的 tokenizer 库）MAY 使用 unsafe，但 MUST 封装在独立 crate 中。
- 与 Python 互操作（PyO3）相关的 unsafe 代码 MAY 使用，但 MUST 遵循 PyO3 官方安全指南。

---

## 第四部分：并发与架构

### 第十条：结构化并发

**Rule**:
所有后台异步任务 MUST 具有明确定义的所有者和生命周期。

每个异步任务 MUST 在启动时明确定义以下属性：

| 属性 | 要求 |
|------|------|
| 启动者（Owner） | 哪个组件 spawn 了此任务 |
| 生命周期（Lifecycle） | 任务何时结束（自然结束、cancellation、shutdown） |
| Cancellation 传播路径 | 如何向此任务传播取消信号 |
| Timeout 策略 | 超时时间及超时行为 |
| Shutdown 行为 | 优雅关闭时任务应完成的操作 |
| Channel 容量 | 有界 channel 的容量及满时策略 |
| Backpressure 策略 | 下游处理慢于上游时的反压机制 |
| Task Join 方式 | 如何等待任务完成并获取结果 |
| 异常传播方式 | 任务 panic 或错误如何向上传播 |

以下行为被严格禁止：

- 无约束的 `tokio::spawn`（即没有具体 owner 的 spawn）
- 永久游离的后台任务（non-terminating orphan tasks）
- 无界 Channel（unbounded channels）
- 忽略 `JoinHandle`（fire-and-forget without error collection）
- 丢弃后台任务的错误
- Session 结束后仍持续运行的 session-scoped 任务

**Rationale**:
Agent 应用本质上是高度并发的——多个 agent 各自运行，互相发送消息，工具调用可能长时间执行。没有结构化并发的并发代码会成为难以调试的 bug 来源。有界 channel 防止内存泄漏，显式 cancellation 保证及时资源回收。

**Enforcement**:
- Code review MUST 验证每个 spawn 有明确的 owner。
- 每个 Session/Agent 对象 MUST 在 Drop 或显式 shutdown 时 cancel 其所有子任务。
- CI 中 MUST 有测试验证：Session 关闭后，相关后台任务在可配置的超时时间内停止。

**Exceptions**:
- 全局共享的基础设施服务（如 Tracing subscriber、全局连接池）MAY 具有应用级生命周期，但 MUST 提供显式 shutdown 函数。

---

### 第十一条：分层与依赖方向

**Rule**:
核心模块（core）MUST NOT 依赖具体模型供应商或基础设施实现。依赖方向 MUST 保持以下约束：

| 层 | 可以依赖 | 禁止依赖 |
|----|---------|---------|
| `core` | 自身 | Provider、具体 HTTP 客户端、具体厂商 |
| `agent` | core | 具体 HTTP 客户端 |
| Model abstraction | core | 具体厂商（如 OpenAI、Anthropic） |
| Tool abstraction | core | MCP 具体实现 |
| Memory abstraction | core | 具体数据库（如 Redis、PostgreSQL） |
| Runtime | 各层 abstraction | MUST NOT 反向污染核心协议 |

Provider、Storage、MCP、Sandbox 和 Observability MUST 通过明确定义的 trait 接口接入核心。

Crate 之间的循环依赖 MUST NOT 存在。

**Rationale**:
依赖倒置是 Rust 生态中保持可维护性的关键。核心协议污染意味着每次加新 provider 都要动核心。抽象使核心稳定、快速编译、独立可测。

**Enforcement**:
- CI MUST 运行依赖图分析（如 `cargo tree` 检查或 `cargo-udeps`），禁止循环依赖。
- PR 中新增的 `depends-on` 关系 MUST 在 review 中说明方向合规性。
- 核心 crate 的 `Cargo.toml` MUST NOT 包含 provider 相关的 features 或依赖。

**Exceptions**:
- `serde`、`tokio`、`tracing` 等框架级依赖 MAY 出现在各层。
- 测试依赖（`dev-dependencies`）MAY 包含 provider 相关 crate，但不影响编译产物方向。

---

## 第五部分：数据与错误协议

### 第十二条：稳定的数据协议

**Rule**:
所有公开数据结构（`#[derive(Serialize, Deserialize)]` 的 public struct/enum）MUST 考虑以下稳定性因素：

- 向后兼容（backward compatibility）
- 未知字段的处理（unknown fields）
- 未知枚举变体的处理（unknown enum variants）
- 新增 ContentBlock 类型的处理
- Provider 扩展字段的处理
- 可选字段的默认值策略
- 序列化格式的版本升级路径

对于可能由上游扩展的枚举（如 Message Role、ContentBlock 类型），SHOULD 提供以下机制之一：

- `#[serde(untagged)]` + catch-all variant（如 `Unknown { ... }`）
- `#[serde(other)]` 变体
- Raw/Extension 字段保留原始 JSON

已发布的字段名称和含义 MUST NOT 随意修改。修改已发布字段 MUST 视为 MAJOR 版本变更。

**Rationale**:
OpenAI、Anthropic 等 LLM Provider 常在 API 中新增字段或内容类型。AgentScope 作为中间层，如果因未知字段而整体反序列化失败，会阻断用户的 agent 运行。稳定数据协议使框架在面对上游演进时保持鲁棒性。

**Enforcement**:
- 序列化往返测试 MUST 验证：带有未预定义字段的 JSON 能否安全反序列化和再序列化。
- `cargo semver-checks` MUST 在 CI 中运行，检测公共 API 的破坏性变更。
- 新增 ContentBlock 类型 MUST 伴随对应的 unknown-field 测试。

**Exceptions**:
- 内部仅用于传递而不序列化的数据结构 MAY 豁免部分要求。
- `#[non_exhaustive]` 标记的 struct 在未达到稳定版本时 MAY 视为不稳定。

---

### 第十三条：稳定错误模型

**Rule**:
所有公开 API 的错误 MUST 满足：

- 类型明确（typed errors, not stringly-typed）
- 可追踪根因（root cause traceable）
- 可转换为稳定的机器可读错误码（stable error codes）
- 区分用户错误、Provider 错误、框架错误和系统错误
- 不泄露 API Key、Token 等敏感信息
- 保留足够的调试上下文（非敏感部分）

MUST NOT 依赖字符串匹配来判断错误类型。

错误类型至少 MUST 区分以下类别：

| 错误类型 | 含义 |
|---------|------|
| `ValidationError` | 输入参数不合法 |
| `ModelError` | 模型调用失败（含 Provider 返回的错误） |
| `ToolError` | 工具执行失败 |
| `TimeoutError` | 操作超时 |
| `CancellationError` | 操作被取消 |
| `PermissionDenied` | 权限不足 |
| `SerializationError` | 序列化/反序列化失败 |
| `SessionError` | 会话状态异常 |
| `UnsupportedFeature` | 暂不支持的功能 |
| `InternalError` | 框架内部错误（bug） |

**Rationale**:
LLM 应用中的错误来源多样——用户输入格式错误、API key 无效、模型超时、token 耗尽、网络中断。用户需要可编程地区分这些情况以做不同处理。字符串匹配式错误处理脆弱且不可靠。

**Enforcement**:
- 每个公共 fallible 函数 MUST 返回类型化错误，而非 `Box<dyn Error>` 或纯字符串。
- CI 中 MUST 有测试：验证稳定错误码的格式和范围。
- 错误信息 MUST 不包含 API Key 或其他高敏感字段（通过 secret-scanning lint）。

**Exceptions**:
- 内部开发者工具（如 migration 脚本）MAY 使用简化错误模型。

---

## 第六部分：可观测性与性能

### 第十四条：可观测性

**Rule**:
所有关键流程 MUST 支持结构化 tracing，并至少包含以下 span/event：

- Session ID
- Agent ID
- Model request（不含敏感内容的请求体）
- Tool invocation（工具名称和参数概要）
- Event publish（事件类型和 sequence number）
- Middleware execution（hook 名称和耗时）
- Retry（重试次数和原因）
- Timeout（超时时长和操作名）
- Cancellation（取消来源）
- Error（错误类型和上下文）
- Token usage（prompt tokens, completion tokens, total）
- Latency（关键操作的 wall-clock 时间）

日志和 trace 数据中 MUST NOT 包含：

- API Key
- Access Token
- 原始密码
- 未脱敏的个人身份信息（PII）
- 默认情况下的完整敏感对话内容

可观测性代码 MUST NOT 改变业务执行顺序（即添加 tracing 不应影响并发调度、cancellation 传播等行为逻辑）。

**Rationale**:
Agent 应用的问题排查非常困难——"为什么 agent 说了这句话"可能涉及多轮模型调用、工具执行和消息传递。结构化 tracing 是唯一可行的调试手段。同时，日志中的敏感信息泄露是安全事件。

**Enforcement**:
- 关键 span 的创建 MUST 通过统一的 tracing 宏进行，CI 可检查是否遗漏。
- Secret scanning 工具 MUST 在 CI 中检查日志输出不包含敏感字段。
- Code review 中 MUST 验证：tracing 的 `.in_scope()` 或 `.entered()` 等调用不会产生可观察的时序变化。

**Exceptions**:
- 开发者 explicitly 启用 debug/trace 级别日志并传入 `--insecure-debug` 标志时，MAY 包含完整对话内容，但 MUST NOT 成为默认行为。

---

### 第十五条：性能不能牺牲正确性

**Rule**:
性能优化的优先级 MUST 遵循以下顺序：

1. 行为正确（Behavioral Correctness）
2. 兼容性正确（Compatibility Correctness）
3. 资源安全（Resource Safety）
4. 可维护性（Maintainability）
5. 性能优化（Performance Optimization）

性能优化 MUST 基于 benchmark 或 profiling 的客观数据，而非主观经验。

以下行为被为优化禁止：

- 改变事件发布顺序
- 忽略或吞并错误
- 移除或跳过 cancellation 检查点
- 破坏用户/Session/Tenant 隔离性
- 修改公开 API 的语义以换取性能
- 引入无法证明安全性的并发优化（如 lock-free 数据结构缺少正确性证明）

**Rationale**:
在最坏情况下，调用方可以通过横向扩展解决性能问题，但无法修复行为正确性问题。AgentScope 的主要瓶颈在 LLM API 调用（网络 I/O），框架开销通常占比较小。优化框架本身应在确认为瓶颈后进行，而不是预先猜测。

**Enforcement**:
- PR 中声称"性能优化"的变更 MUST 附 benchmark 数据。
- 性能 benchmark MUST 在 CI 中运行并记录历史趋势。
- 任何改变并发模型或事件顺序的 PR MUST 标注为高风险并强制额外审查。

**Exceptions**:
- 编译时间优化 MAY 在无 profiling 数据时进行（如依赖瘦身），但 MUST NOT 改变运行时行为。

---

## 第七部分：交付与治理

### 第十六条：小步交付

**Rule**:
项目 MUST 按独立能力模块拆分交付，MUST NOT 试图在单个 specification 中实现完整 AgentScope。

建议的能力拆分如下（顺序和边界 MAY 调整）：

1. Compatibility baseline（兼容性基准与测试基础设施）
2. Message model（消息与内容块模型）
3. Event system（事件系统）
4. Model API（模型调用抽象）
5. Tool system（工具系统）
6. Agent loop（Agent 推理-行动循环）
7. Streaming（流式响应）
8. Memory（内存/记忆管理）
9. Middleware（中间件管道）
10. Session（会话管理）
11. RAG（检索增强生成）
12. Workspace（工作空间管理）
13. Sandbox（代码执行沙箱）
14. Multi-agent（多 Agent 协作）
15. Distributed runtime（分布式运行时）

每个 feature MUST 能够独立完成以下流程：

- 编写 specification
- 实现代码
- 运行测试
- 通过验收
- 生成兼容性报告

**Rationale**:
"实现完整 AgentScope"是一个巨大而模糊的目标。拆分为独立能力模块使每步可规划、可测试、可验收。每个模块完成后立即产生用户价值。

**Enforcement**:
- 每个 feature specification MUST 声明其前置依赖以及与其他模块的接口边界。
- 项目管理中 MUST 维护每个模块的实现状态。
- 未经拆分的、"一个大 specification 覆盖所有"的提案 MUST 被拒绝。

**Exceptions**:
- 基础设施类能力（如 tracing、错误类型定义）MAY 与首个业务模块并行实现。

---

### 第十七条：完成的定义

**Rule**:
一项功能只有在满足以下所有条件时才 MAY 被标记为"完成"：

- [ ] Specification 已获批准
- [ ] Implementation Plan 与 Constitution 一致
- [ ] 所有 Tasks 已完成
- [ ] Rust 单元测试通过
- [ ] P0 差分测试（Rust vs Python）通过
- [ ] 无未记录的兼容性偏差
- [ ] 无静默降级行为
- [ ] 文档已更新（API docs + 用户文档）
- [ ] 示例代码可编译运行
- [ ] 兼容性矩阵已更新
- [ ] `cargo clippy` 无警告
- [ ] `cargo fmt` 检查通过
- [ ] 公共 API 变化已经过独立的 API review
- [ ] 不存在未登记的 `UnsupportedFeature`

"代码已经写完"不等于功能完成。

**Rationale**:
未经验证的代码不是可交付的功能。"差不多实现了"在兼容性项目中没有意义——不完整的功能就是不可靠的功能。完整的 Done Definition 使各方对"完成"有统一认知。

**Enforcement**:
- PR 模板 MUST 包含此 checklist。
- 合并代码的审批人 MUST 逐项确认 checklist。
- CI MUST 自动检查 clippy、fmt、tests，未通过则阻断合并。

**Exceptions**:
- 纯文档变更 MAY 豁免部分 checklist（跳过测试、clippy 等），但仍需 API review 和格式检查。

---

### 第十八条：兼容性分级

**Rule**:
兼容性 MUST 按以下四级声明，每个模块 MUST 明确标注当前达到的兼容等级，MUST NOT 笼统宣称"完全兼容"：

| 等级 | 名称 | 定义 |
|------|------|------|
| **L1** | 协议兼容 | 数据结构定义、序列化/反序列化格式和基础协议与 Python 实现保持兼容 |
| **L2** | 核心行为兼容 | Agent、Model、Tool、Event、Memory 等核心流程的外部可观察行为兼容 |
| **L3** | 公开 API 语义兼容 | 主要公开接口拥有等价的 Rust API，且接口行为语义与 Python 等价 |
| **L4** | 示例迁移兼容 | AgentScope 官方示例可以低成本（API 名称变化 + 语言差异）迁移到 Rust 实现 |

每个模块的兼容等级 MUST 在兼容性矩阵（Compatibility Matrix）中记录，并在发布说明中引用。

**Rationale**:
"完全兼容"是一个没有信息量的声明。L1-L4 的分级使调用方能精确了解他们可以依赖什么——有些用户只需要 L2 兼容就可以迁移，有些需要 L4。

**Enforcement**:
- 每个模块的 specification MUST 包含目标兼容等级。
- 发布时 MUST 生成兼容性报告，标明每个模块的实际等级。
- 如果一个模块自称为 L4 但示例无法迁移，MUST 降级该模块。

**Exceptions**:
- 纯内部模块（如日志基础设施）MAY 不适用此分级。

---

### 第十九条：变更治理

**Rule**:
任何违反本宪法的设计决策 MUST 经过以下流程：

1. 明确说明违反的条款（引述条款编号和名称）
2. 说明无法遵守该条款的客观原因
3. 提供替代方案（为什么替代方案不可行）
4. 记录由此引入的风险
5. 经过人工批准（非自动流程）

修改 Constitution 本身 MUST 经过以下流程：

1. 提供修改原因和动机
2. 评估对已有 specification 和已实现代码的影响
3. 更新 Constitution 版本号（遵从语义化版本规则）
4. 更新生效日期
5. 检查所有现有模块是否仍然符合修改后的要求
6. 如果不符合，列出需要更新的模块及更新计划

**Rationale**:
宪法是一份活文件，需要随着项目演进而调整，但其稳定性是可预测性的基础。变更治理防止随意的、未经评估的宪法修改破坏项目的一致性。

**Enforcement**:
- 宪法修改 MUST 以独立 PR 提交，不与其他代码变更混合。
- 宪法版本号 MUST 记录在文件末尾。
- 任何声称"违反宪法但已批准"的设计决策 MUST 在相关代码中注释引述审批记录。

**Exceptions**:
- 宪法的拼写错误修正 MAY 不经过完整流程，但仍需更新 PATCH 版本号。

---

## 治理

本宪法是 AgentScope Rust 项目的最高权威文件。

- 所有设计决策、代码审查、Pull Request 和发布流程 MUST 以本宪法为最终仲裁依据。
- 当本宪法与其他团队惯例发生冲突时，本宪法优先。
- 每个 specification 的 "Constitution Check" 章节 MUST 声明该 specification 与各条款的符合性。
- 开发者发现本宪法的条款在实际情况中不合理或不可行时，MUST 通过变更治理流程提出修改提案，而非私下忽略条款。

**合规审查周期**: 每个发布周期（milestone）结束时 MUST 进行一次宪法合规审查，检查最近一个周期内：

1. 是否存在已批准的宪法违反豁免
2. 豁免是否仍然有效（风险是否已变化）
3. 新代码是否符合当前宪法版本

**版本语义**: 宪法版本号遵循 `MAJOR.MINOR.PATCH`：
- **MAJOR**：向后不兼容的治理变更、原则移除或重新定义
- **MINOR**：新增原则/章节，或对现有指导的实质性扩展
- **PATCH**：澄清表述、修正笔误、非语义性的文字优化

**版本**: 1.0.0 | **生效日期**: 2026-07-28 | **最后修订**: 2026-07-28
