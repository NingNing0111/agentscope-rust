# Research: Complete Agent Demo

**Date**: 2026-08-01 | **Feature**: [spec.md](./spec.md)

本文件汇总 Phase 0 的技术决策。Feature 019 的目标是新增 `examples/agent-demo` canonical Agent demo；研究重点为 example 包装方式、确定性验证路径、live provider 分离、能力覆盖边界、trace/secret 策略与维护验证方式。

## D1: 示例载体 —— `examples/agent-demo/main.rs` 目录型 example + 显式 Cargo 登记

- **Decision**: 新增 `examples/agent-demo/` 目录，CLI 入口为 `main.rs`；根 `Cargo.toml` 添加 `[[example]] name = "agent_demo" path = "examples/agent-demo/main.rs"`。
- **Rationale**: Demo 需要 README、fixtures、trace schema、deterministic/live runner、tools、middleware 等多个文件。目录型 example 保持关注点清晰，也满足用户指定的 `examples/agent-demo` 位置。项目根 `Cargo.toml` 当前 `autoexamples = false`，现有示例均通过 `[[example]]` 显式注册，因此新 demo 必须同样显式登记，才能被 `cargo build --examples` 覆盖。
- **Alternatives considered**: 单文件 `examples/agent_demo.rs`（无法自然承载 README/fixtures，多能力展示会形成超大文件）；新增 workspace crate（扩大生产依赖面，不符合示例性质）；把逻辑塞进现有 `chat.rs`（不满足 canonical complete demo 的独立入口要求）。

## D2: 默认运行路径 —— deterministic/offline 为主，live-model 为 opt-in

- **Decision**: `--mode deterministic` 是默认或推荐验证路径，不依赖网络、真实 LLM 或 API key；`--mode live` 仅在用户明确选择并提供 `API_KEY`/model 后运行 DashScope provider。
- **Rationale**: 宪法第六条要求核心兼容性不得依赖真实 LLM 自然语言输出；spec FR-012 要求 mockable/deterministic path，FR-013 要求 optional live-model path。默认 deterministic 模式可保证 maintainer validation 稳定，live 模式则展示真实 provider 集成但不承担唯一验收责任。
- **Alternatives considered**: 只提供 live demo（不稳定、依赖网络与凭据，违反 FR-012）；只提供 deterministic demo（无法展示真实 provider onboarding，弱化 FR-013）；录制真实 provider 响应作为唯一路径（仍需维护录制资产，且不如 scripted scenario 清晰）。

## D3: 场景设计 —— 单一连贯任务串联多个能力，而非多个孤立 mini-demo

- **Decision**: 主场景采用一个可解释的开发者任务，例如“让 Agent 帮助分析一段需求、调用计算/检索工具、读取上下文、写入 workspace artifact、跨第二轮继续回答并输出总结”。每个步骤在 trace 中映射到 capability coverage item。
- **Rationale**: 用户要求“完整 Agent 示例”与“所有功能体现在这个 demo 里”；单一 walkthrough 比多个碎片脚本更能展示 AgentScope 的集成体验，也便于 SC-007 让 maintainer 对照 checklist 验证。
- **Alternatives considered**: 每个能力一个独立 command（覆盖清晰但不是完整 Agent flow）；只展示 chat + tool（太窄，无法覆盖 memory/session/RAG/workspace/sandbox/trace）；试图展示项目所有内部实现细节（范围爆炸，违反小步交付）。

## D4: 能力覆盖边界 —— 展示“主要 Agent demo 能力”，未实现或不适合默认运行的能力显式标注

- **Decision**: Demo coverage 分为 `demonstrated`、`optional`、`skipped`、`unsupported` 四类。默认 deterministic 主路径至少 demonstrated：Agent interaction、structured messages、events/progress、streaming-like incremental output、tool invocation、session continuity、memory/context recall、middleware/trace、typed error handling；RAG/workspace/sandbox 根据已实现 API 以 deterministic 或 documented optional 方式展示。Multi-agent 与 Distributed runtime 不纳入本 feature 默认覆盖（尚未作为完成能力），在 README 中作为 non-goal/roadmap 标注。
- **Rationale**: spec assumptions 已将“所有功能”解释为“all major capabilities currently relevant to an Agent demonstration”。宪法第五条禁止伪兼容；无法安全演示的平台能力必须清楚呈现状态，而不是空实现冒充成功。
- **Alternatives considered**: 强行把所有 roadmap feature 写进 demo（会制造伪兼容）；只覆盖最小 happy path（不满足 FR-011 capability checklist）；默认执行沙箱/外部服务副作用（安全与环境要求过高）。

## D5: Trace 产物 —— 结构化、可读、默认脱敏

- **Decision**: Demo 输出双层 trace：终端 human-readable timeline + 可选 `--trace-json <path>` 写出结构化 JSON。Trace event 包含 step id、capability ids、event kind、safe summary、status、sanitized metadata，不包含 raw secret。默认不记录完整敏感 conversation；如需 verbose，必须由显式 flag 启用并仍脱敏 credentials。
- **Rationale**: 宪法第七条将 trace 视为核心验收产物，第十四条要求日志不泄露 secrets。结构化 trace 支持 maintainer regression 验证，human timeline 支持新用户学习。
- **Alternatives considered**: 只打印 final answer（无法证明能力覆盖）；完整 dump 内部对象（泄密/噪声风险）；只写 JSON 不打印 timeline（学习体验差）。

## D6: CLI 契约 —— 少量稳定参数覆盖验证、live、错误演示与输出控制

- **Decision**: CLI 最小参数集：`--mode deterministic|live`、`--api-key`/`API_KEY`、`--model`、`--trace-json`、`--show-coverage`、`--fail-tool`、`--cancel-after-step`、`--workspace-dir`、`--verbose`。缺配置时返回非零 exit code 与 actionable message。
- **Rationale**: 参数直接映射 FR-002/014/017：可运行说明、optional live、tool failure/cancellation validation、trace output 与 secret-safe diagnostics。参数过多会降低学习性；过少无法验证边界情况。
- **Alternatives considered**: 无 CLI 参数、只靠环境变量（不可发现）；完整 production-grade 配置文件（超出示例范围）；交互式 prompt（不利于 CI/自动验证）。

## D7: 错误与取消策略 —— 演示可恢复错误，不制造不可控副作用

- **Decision**: Demo 提供 deterministic 的 tool failure injection 与 cancellation injection，并将错误分类到 `config_error`、`tool_error`、`model_error`、`unsupported_feature`、`cancelled`、`internal_error` 等稳定类别。默认 workspace 写入只发生在可清理 demo 目录，且启动时报告路径。
- **Rationale**: FR-014 要求 missing config、tool failures、cancellation 清晰可见；宪法第十三条要求 typed/stable error model。受控注入比依赖随机失败更适合 regression。
- **Alternatives considered**: 只覆盖 happy path（边界不可验证）；用 panic 模拟失败（违反错误模型）；写入用户当前目录且不提示（副作用不透明）。

## D8: README 与 coverage checklist —— README 是学习入口，contracts checklist 是验证入口

- **Decision**: `examples/agent-demo/README.md` 包含 setup、deterministic run、live run、expected output、coverage checklist、limitations/non-goals、troubleshooting。Spec contract `coverage-checklist.md` 定义实现阶段 README 与 runtime `--show-coverage` 必须覆盖的能力项。
- **Rationale**: FR-002/011/016/017 都要求文档化 setup、能力映射、边界与验证。将 checklist 同时体现在 README 和运行输出中，用户无需读源码即可确认覆盖。
- **Alternatives considered**: 只在 spec 中维护 checklist（用户看不到）；只在 README 自由书写（实现验证不稳定）；把 coverage 隐藏在测试断言中（学习价值低）。

## D9: 不新增生产 API 优先，必要 helper 局限在 example 内

- **Decision**: 实现优先复用已公开 crate API 和 `examples/common.rs` 的模式；若需要 scripted model、trace renderer、demo tool 等 helper，优先放在 `examples/agent-demo/` 内，不新增或改变生产 crate API。
- **Rationale**: Demo 是对既有能力的 showcase，不应为了示例扩大公共 API 面。example-local helper 可快速迭代且不会造成 semver/compatibility 负担。
- **Alternatives considered**: 为 demo 新增 core helper API（需要独立 spec/API review）；复制大量现有 examples 代码但不整理（维护成本高）；直接依赖 crate 内部模块（破坏封装）。

## 结论

全部 9 项决策落定，Technical Context 无遗留 NEEDS CLARIFICATION，可进入 Phase 1 设计。