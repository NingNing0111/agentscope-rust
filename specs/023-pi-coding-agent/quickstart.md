# Quickstart: Pi Coding Agent (Rust)

**Feature**: 023-pi-coding-agent
**Purpose**: End-to-end validation guide for the pi-rust coding Agent.

## Prerequisites

- Rust toolchain installed and usable from the repository root.
- A valid DashScope API key for real-provider validation.
- Repository checked out at `agentscope-rust` root.

Set credentials:

```bash
export API_KEY="sk-your-key"
```

## Build/Check

From repository root:

```bash
cargo check -p pi-rust
```

Expected outcome:

- Command exits successfully.
- No compile errors.

## Run Help

```bash
cargo run -p pi-rust -- --help
```

Expected outcome:

- CLI prints available options from [CLI Contract](./contracts/cli-contract.md).
- Output does not print API keys.

## One-shot Prompt Validation

```bash
cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"
```

Expected outcome:

- CLI starts without entering REPL.
- Agent streams a concise answer.
- Process exits with code 0.
- Session is saved under the configured workdir.

## Read Tool Validation

```bash
cargo run -p pi-rust -- --prompt "请读取 examples/pi-rust/src/main.rs，并说明它当前做了什么。"
```

Expected outcome:

- Agent calls `Read`.
- Output mentions the file's current behavior.
- If events are enabled, the event stream shows a Read tool invocation and result.

## Write Tool Validation

Use a disposable workdir and project directory:

```bash
mkdir -p /tmp/pi-rust-quickstart/project
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --prompt "创建 hello.txt，内容是 Hello, World!"
```

Expected outcome:

- Agent calls `Write`.
- `/tmp/pi-rust-quickstart/project/hello.txt` exists.
- File content is exactly or semantically equivalent to `Hello, World!` as requested.

## Edit Tool Validation

```bash
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --prompt "把 hello.txt 里的 World 改成 Rust。"
```

Expected outcome:

- Agent calls `Read` and/or `Edit`.
- `hello.txt` contains `Hello, Rust!`.
- If the target string is ambiguous or absent, the tool reports the corresponding error from [Tool Contracts](./contracts/tool-contracts.md).

## Bash Tool Validation

```bash
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --prompt "执行 pwd，并告诉我返回了什么。"
```

Expected outcome:

- Agent calls `Bash`.
- Output includes `/tmp/pi-rust-quickstart/project`.
- Command output is summarized without unrelated noise.

## Session Resume Validation

First run:

```bash
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --prompt "请记住：这个 quickstart 项目的问候语是 Hello Rust。"
```

Then resume interactively:

```bash
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --resume
```

In the REPL, ask:

```text
刚才我让你记住的问候语是什么？
```

Expected outcome:

- CLI resumes the previous session or loads the latest session.
- Agent can answer using persisted context and/or memory.

## Error Handling Validation

Run with a missing API key:

```bash
API_KEY= cargo run -p pi-rust -- --prompt "hello"
```

Expected outcome:

- CLI exits with code 2.
- Error clearly explains how to provide credentials.
- No panic/backtrace is shown in normal mode.

## Safety Validation

Ask the Agent to delete a file:

```bash
cargo run -p pi-rust -- \
  --workdir /tmp/pi-rust-quickstart/state \
  --cwd /tmp/pi-rust-quickstart/project \
  --prompt "删除 hello.txt"
```

Expected outcome:

- If implemented via Bash or file mutation, the operation requires confirmation before destructive execution.
- If confirmation is denied, file remains unchanged.

## Cleanup

```bash
rm -rf /tmp/pi-rust-quickstart
```

## Validation Checklist

- [ ] `cargo check -p pi-rust` passes.
- [ ] CLI help matches contract.
- [ ] One-shot prompt works.
- [ ] Read tool works.
- [ ] Write tool works.
- [ ] Edit tool works.
- [ ] Bash tool works.
- [ ] Session resume works.
- [ ] Missing credentials produce user-friendly error.
- [ ] Destructive operations require confirmation.
