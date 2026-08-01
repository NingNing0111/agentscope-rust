# Quickstart: Complete Agent Demo

**Date**: 2026-08-01 | **Feature**: [spec.md](./spec.md)

本文定义 Feature 019 完成后的验证路径。当前为设计阶段 quickstart；实现完成后，命令应可直接运行。

## Prerequisites

- Rust toolchain compatible with workspace edition 2024
- Repository dependencies available locally
- Optional live mode only: DashScope API key via `API_KEY` or `--api-key`

Deterministic validation does **not** require network access or model credentials.

## 1. Build all examples

```bash
rtk cargo build --examples
```

Expected:

- `agent_demo` example is included in build output because root `Cargo.toml` explicitly registers it.
- Build completes without errors.

## 2. Run deterministic primary scenario

```bash
rtk cargo run --example agent_demo -- --mode deterministic --show-coverage
```

Expected high-level output:

- Demo title and `mode: deterministic`
- Preflight states no API key is required
- Scenario timeline appears in step order
- Tool invocation is visible with safe argument summary
- At least two turns demonstrate session continuity
- Memory/context recall evidence is visible
- Middleware/trace events are visible
- Final summary is printed
- Coverage checklist reports required demonstrated capabilities

## 3. Write and inspect trace JSON

```bash
rtk cargo run --example agent_demo -- \
  --mode deterministic \
  --show-coverage \
  --trace-json target/agent-demo/trace.json
```

Expected:

- File `target/agent-demo/trace.json` is written.
- JSON follows [contracts/trace-schema.md](./contracts/trace-schema.md).
- `events[].sequence` is ordered.
- `coverage[]` contains the same capability ids as [contracts/coverage-checklist.md](./contracts/coverage-checklist.md).
- No raw secret values appear.

## 4. Validate missing live configuration handling

Run without credentials:

```bash
rtk cargo run --example agent_demo -- --mode live
```

Expected:

- Command fails before any provider call.
- Exit code is a configuration/usage error.
- Output explains how to set `API_KEY` or pass `--api-key`.
- Output suggests deterministic mode for no-credential validation.
- No secret value is printed.

## 5. Optional live run

```bash
API_KEY=sk-... rtk cargo run --example agent_demo -- \
  --mode live \
  --model qwen-plus \
  --show-coverage
```

Expected:

- Preflight reports live mode and masks credentials.
- Current implementation validates the opt-in live boundary and returns a categorized `model_error` skeleton instead of making provider natural-language output part of deterministic regression evidence.
- Use `examples/chat.rs` for a fully interactive DashScope streaming chat path.

## 6. Validate tool failure path

```bash
rtk cargo run --example agent_demo -- --mode deterministic --fail-tool --show-coverage
```

Expected:

- Tool call is attempted.
- Tool failure is reported with a stable category/code.
- Trace contains `tool_called` and `tool_failed` or an equivalent stable event sequence.
- Existing trace before the failure remains visible.

## 7. Validate cancellation path

```bash
rtk cargo run --example agent_demo -- \
  --mode deterministic \
  --cancel-after-step tool-use \
  --trace-json target/agent-demo/cancelled-trace.json
```

Expected:

- Run terminates as cancelled (recommended exit code 130).
- Trace contains `cancellation_requested`.
- Events from completed steps before cancellation are preserved.

## 8. Maintainer final checks

Before marking Feature 019 complete, run:

```bash
rtk cargo fmt --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo build --examples
rtk cargo run --example agent_demo -- --mode deterministic --show-coverage --trace-json target/agent-demo/trace.json
```

Completion evidence must include:

- `examples/agent-demo/README.md` exists and documents deterministic/live paths.
- Root `Cargo.toml` registers `agent_demo`.
- Deterministic scenario completes without credentials.
- Coverage checklist maps at least 8 major capabilities to observable evidence.
- Default output and trace contain no raw secrets.
- Optional/unsupported capabilities are clearly labeled and not counted as demonstrated.
