# Release-Gate Requirements Quality Checklist: Agent System

**Purpose**: 发布门 — 验证 Feature 007 Agent System 需求的完整性、清晰度、一致性和可度量性。面向作者自查，全领域覆盖（Agent trait / ReAct Loop / Middleware / Interruption / Context Compression / Permission）。
**Created**: 2026-07-29
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md) | [data-model.md](../data-model.md)

**Note**: 此检查清单由 `/speckit-checklist` 命令生成。每项检查需求规范的写法质量——而非验证实现行为。

---

## 一、需求完整性（Requirement Completeness）

- [ ] CHK001 — Agent trait 的 `reply()` 输入类型定义为 `Msg | Vec<Msg> | None`，是否对所有三种输入变体的具体处理语义都有明确需求？[Completeness, Spec §FR-002, FR-003]
- [ ] CHK002 — `reply_stream()` 返回的 `Stream<Item = AgentEvent>` 是否明确定义了流的生命周期（何时关闭、错误时是否 abort）以及是否需要同时返回 `Msg`？[Completeness, Spec §FR-003]
- [ ] CHK003 — `observe()` 被定义为 `Msg | Vec<Msg> | None`，是否对 "在 reply 进行中调用 observe()" 这一并发场景的行为有明确需求？[Completeness, Edge Case, Spec §Edge Cases]
- [ ] CHK004 — Context compression 在触发后替换了旧消息为 `SummaryContent block`，但是否定义了压缩后的 SummaryContent block 的格式和元数据要求？[Completeness, Spec §FR-021]
- [ ] CHK005 — PermissionEngine 的 `RequireUserConfirm` 状态——等待外部确认的机制（超时？默认行为？）是否有明确需求？[Completeness, Spec §FR-025, FR-026]
- [ ] CHK006 — `AgentState` 的序列化和反序列化行为是否有需求定义？（data-model 中标注 state 字段为 `RwLock<AgentState>`，但未定义 persistence 行为）[Completeness, Gap]
- [ ] CHK007 — `AgentConfig::system_prompt` 的注入位置（在消息列表的何处插入 system_prompt）是否有明确定义？[Completeness, Spec §FR-012]
- [ ] CHK008 — `structured_output_grace_iters` 的超时或最大尝试次数之外，structured output 失败后的回退策略（是否降级到普通文本回复）是否有需求定义？[Completeness, Spec §FR-011]

## 二、需求清晰度（Requirement Clarity）

- [ ] CHK009 — FR-010 中 "events in the order defined by AgentScope protocol" —— 是否引用了具体的协议规范文档或提供了完整的事件顺序状态机？[Clarity, Spec §FR-010]
- [ ] CHK010 — FR-020 中 "monitor context length against trigger_ratio" —— "context length" 的度量单位是什么（token count? 字符数? 消息数？），是否与 `ChatModel::count_tokens()` 对齐？[Clarity, Spec §FR-020]
- [ ] CHK011 — FR-021 中 "summarize older messages" —— "older" 的判定标准是什么？是按消息时间戳、位置索引还是 token 占比？[Clarity, Spec §FR-021]
- [ ] CHK012 — FR-022 中 `trigger_ratio` 的分母是什么（模型 context window 总 token 数？），分母从哪里获取？[Clarity, Spec §FR-022]
- [ ] CHK013 — SC-004 中 "configurable grace period (default 5 seconds)" —— grace period 的配置入口在哪里？`ReActConfig` 和 `ContextConfig` 中都没有此字段。[Clarity, Spec §SC-004]
- [ ] CHK014 — `Middleware` trait 有 8 个 hook 方法，但 types crate 有 10 个 hook 常量（data-model §7 注明了映射关系），这种数量不匹配的理由和设计决策是否在 spec 中明确记录？[Clarity, data-model §7]
- [ ] CHK015 — FR-025 中 "check permissions via PermissionEngine" —— PermissionEngine 是从 `agent_scope_state` 还是 `agent_scope_agent` 导入？data-model §9 和 tasks.md T069 暗示了迁移，但 spec 未明确声明来源。[Clarity, Spec §FR-025]
- [ ] CHK016 — `ContextConfig::compression_prompt` 的默认值 `"<STD_CP_PROMPT>"` 是否为实际可用的 prompt，还是一个需要用户自行替换的占位符？[Clarity, data-model §3]

## 三、需求一致性（Requirement Consistency）

- [ ] CHK017 — data-model §7 定义 middleware 通过 `&ReActAgent` 参数传递，但 FR-016 仅说 Middleware trait 有 hook 方法——middleware 能否访问 agent 内部状态（如 messages、toolkit）？spec 和 data-model 之间是否一致？[Consistency, Spec §FR-016 vs data-model §7]
- [ ] CHK018 — spec 的 Assumptions 说 `PermissionEngine` 将在本 feature 中完全实现，但 plan.md Constitution Check 中 Article 5 又说 "PermissionEngine placeholder will be replaced" —— 这两个声明是否一致？实际上 T065 实现了 PermissionEngine 在 agent crate 中，而 T069 替换了 state crate 的占位符。[Consistency, Spec §Assumptions vs plan.md]
- [ ] CHK019 — FR-012 定义 `AgentConfig.toolkit` 类型为 `Option<ToolKit>`，但 FR-006-FR-011 的 ReActAgent 与 tool 的交互是通过 ToolKit 还是通过 `Tool` trait？spec 未明确两者关系。[Consistency, Spec §FR-012 vs FR-008]
- [ ] CHK020 — SC-005 要求 "Context compression reduces token count to within configurable bounds"，但 `ContextConfig` 中没有定义压缩后的目标 token 数边界（只有 trigger_ratio 和 reserve_ratio）。压缩到多少才算 "within configurable bounds"？[Consistency, Spec §SC-005 vs data-model §3]
- [ ] CHK021 — SC-006 要求 "at least 3 reasoning-acting iterations without state corruption"，但 `ContextConfig` 和 `InjectionConfig` 中都没有定义如何验证 state 未被破坏——是否有具体的 state integrity 检查需求？[Consistency, Spec §SC-006]

## 四、验收标准质量（Acceptance Criteria Quality）

- [ ] CHK022 — US1 验收场景 1 中 "events in the order: ReplyStart → ModelCallStart → ..." —— 是否定义了该事件序列的非确定性因素（如多个 TextBlockDelta 的数量）以及哪些差异可接受？[Acceptance Criteria, Spec §US1-AS1]
- [ ] CHK023 — US2 验收场景 3 中 "ToolResultEnd with state=execution_error" —— `state` 字段的完整枚举值集合是否在需求中定义？是否只有 `execution_error` 而没有其他 ToolResultEnd 状态？[Acceptance Criteria, Spec §US2-AS3]
- [ ] CHK024 — US3 验收场景 2 中 "middleware can modify or reject the tool execution" —— "reject" 的具体含义是什么？是返回 Err、修改 tool_call 参数还是设置一个拒绝标志？可观测行为是什么？[Acceptance Criteria, Spec §US3-AS2]
- [ ] CHK025 — SC-001 中 "fewer than 10 lines of setup code" —— 这 10 行是否包含 use 语句和导入？计数标准是否明确？[Measurability, Spec §SC-001]
- [ ] CHK026 — SC-002 中 "10+ event types for a complete ReAct cycle" —— "10+" 是否是一个需求要求（规范性的），而非一个观察性描述（信息性的）？[Measurability, Spec §SC-002]

## 五、场景覆盖（Scenario Coverage）

- [ ] CHK027 — FR-023 定义了单个 `UserInterruptEvent` 的处理，但如果有多个并发的中断信号（如同时收到 user interrupt 和 system shutdown），行为是否定义？[Coverage, Spec §FR-023]
- [ ] CHK028 — `reply_stream()` 在 streaming 过程中被中断——UI 下游取消订阅（drop Receiver）的行为是否与 UserInterruptEvent 有区别需求？[Coverage, Spec §FR-003 vs FR-023]
- [ ] CHK029 — 模型调用返回的 `ChatResponse` 中同时包含 text content 和 tool call content（混合响应），agent 的处理优先级和顺序是否有需求？[Coverage, Gap]
- [ ] CHK030 — Context compression 发生后，如果新消息继续涌入，是否定义了二次压缩的触发条件？（连续压缩会无限循环吗？）[Coverage, Spec §FR-020–FR-022]
- [ ] CHK031 — PermissionEngine 的 `RequireUserConfirm` 等待外部确认时，是否有超时需求？超时后是默认拒绝还是默认允许？[Coverage, Spec §FR-026]
- [ ] CHK032 — `structured_output` 模式下模型返回的 JSON 不符合 schema——每轮重试是否计入 `max_iters`？是否会因为结构化输出重试消耗掉所有迭代额度而导致无法进行正常的 reasoning-acting？[Coverage, Spec §FR-011]

## 六、边界情况覆盖（Edge Case Coverage）

- [ ] CHK033 — spec §Edge Cases 列出了 6 个边界情况，但 plan.md 和 tasks.md 中是否已为每个边界情况分配了具体的测试任务？[Edge Case Coverage, Spec §Edge Cases]
- [ ] CHK034 — "observe() 在 reply 进行中调用"（Edge Case #6）——tasks.md T076 标注为 "(queue or error)"，说明行为尚未最终确定。规范中是否应该先确定行为？[Edge Case, Spec §Edge Cases vs tasks.md T076]
- [ ] CHK035 — 当所有 tool 都被 PermissionEngine 拒绝且 `stop_on_reject=true` 时，agent 是返回 `AgentError::PermissionDenied` 还是返回一个包含拒绝信息的 Msg？[Edge Case, Spec §FR-026]
- [ ] CHK036 — `middlewares` 向量为空（未注册任何 middleware）时的行为是否有明确需求？[Edge Case, Gap]

## 七、非功能需求（Non-Functional Requirements）

- [ ] CHK037 — plan.md 定义 "Agent loop overhead < 10ms per iteration"，此性能目标是否在 spec 中作为非功能需求明确记录？是否有对应的验收标准？[NFR, plan.md vs spec.md]
- [ ] CHK038 — plan.md 定义 "No unbounded memory growth — AgentState::context bounded by configurable limits" —— `AgentState::context` 的消息数量或 token 上限是否在 spec 的需求中定义？[NFR, plan.md vs spec.md]
- [ ] CHK039 — plan.md Constitution Check Article 14 声称 "Structured tracing via `tracing` crate for model calls, tool invocations, hook execution, errors, token usage" —— coverage 中有哪些 hook 点产 tracing span？tracing 粒度需求是否在 spec 中记录？[NFR, plan.md vs spec.md]
- [ ] CHK040 — `EventEmitter` 定义使用 `tokio::sync::broadcast` channel（data-model §8），当 lagged 时 "drops if no receivers with lagged warning"——是否有 slow consumer 的处理策略（如 event 丢失是否可以恢复）？[NFR, data-model §8]

## 八、依赖与假设（Dependencies & Assumptions）

- [ ] CHK041 — spec §Assumptions 声明 "Mock/scripted model implementations exist in test utilities and are NOT part of this feature's deliverable" —— 但 tasks.md T012/T013 直接将 MockModel/ScriptedModel 作为本 feature 的任务。这是否意味着这些 mock 是本 feature 应交付的基础设施？[Assumption, Spec §Assumptions vs tasks.md T012-T013]
- [ ] CHK042 — spec §Assumptions 声明 "Runtime state injection (time, tasks, context length) via InjectionConfig is deferred" —— 但 `ReActAgent` struct（data-model §6）中没有 `InjectionConfig` 字段，这确认了推迟，但 spec 中没有说明 InjectionConfig 未来如何集成。[Assumption, Spec §Assumptions]
- [ ] CHK043 — spec §Assumptions 声明 "Context compression uses the agent's own model" —— 如果 agent 的 model 也是即将被压缩的上下文的来源，是否存在自我指涉的可靠性问题？这个假设是否有风险评估？[Assumption, Spec §Assumptions]
- [ ] CHK044 — tasks.md T069 "Replace PermissionContext placeholder in agent_scope_state with real PermissionEngine or re-export from agent_scope_agent" —— 这个跨 crate 修改是否在 spec 的依赖关系或假设中声明？[Dependency, tasks.md T069]

## 九、可追溯性（Traceability）

- [ ] CHK045 — FR-001 到 FR-026 是否都有对应的测试任务？可以对照 tasks.md 检查每个 FR 的覆盖率。[Traceability]
- [ ] CHK046 — 4 个 User Story 是否都对齐了独立的 success criteria（SC-001 到 SC-006）？US1-US4 与 SC-001 到 SC-006 的映射关系是否明确？[Traceability, Spec §US1-US4 vs §SC-001–SC-006]
- [ ] CHK047 — spec §Edge Cases 中的 6 个边界情况是否都在 FR-001 到 FR-026 中有对应的功能需求引用？[Traceability, Spec §Edge Cases]
- [ ] CHK048 — data-model 中定义的所有实体（9 个）是否都在 spec 的 Key Entities 和 Functional Requirements 中有对应引用？[Traceability, data-model vs spec.md]

## 十、模糊点与冲突（Ambiguities & Conflicts）

- [ ] CHK049 — FR-003 "return a Stream yielding AgentEvent items (and optionally the final Msg)" —— "optionally the final Msg" 意味着某些情况下不返回 final Msg，什么时候不返回？[Ambiguity, Spec §FR-003]
- [ ] CHK050 — FR-009 "returning the last model response when exceeded" —— "last model response" 可能是一个 tool call 而非 text response，这种情况下返回给调用方的内容是什么？[Ambiguity, Spec §FR-009]
- [ ] CHK051 — FR-019 "Hooks MUST be invoked in registration order (FIFO) per hook point" —— 如果一个 middleware 的 `pre_reply` 返回了 Err，后续 middleware 的 `pre_reply` 是否还会被调用？[Ambiguity, Spec §FR-019]
- [ ] CHK052 — data-model §8 EventEmitter::emit "drops if no receivers with lagged warning" —— lagged 的判断标准是什么？lag 多少个事件算 lagged？这是需求还是实现细节？[Ambiguity, data-model §8]

---

## Notes

- **检查清单类型**: 发布门（Release Gate）—— 严格深度，覆盖所有 6 个能力域
- **受众**: 作者自查
- **现有文件**: 追加模式（新文件 `release-gate.md`，不与 `requirements.md` 冲突）
- **条目数**: 52 条（CHK001–CHK052），追溯覆盖率：≥88%（46/52 含 Spec/plan/data-model/Edge Case 引用）
- 所有条目均测试需求的写法质量，不验证实现行为
