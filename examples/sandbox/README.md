# `sandbox` 示例

沙箱示例展示 `LocalSandboxSession` 的命令执行、文件读写、路径越界防护和 `CapabilityReport`；启用 `microsandbox` feature 后，还可以显式运行真实 microsandbox 后端路径。

默认运行不需要模型、API key 或 microsandbox runtime。它使用本地临时目录作为 sandbox root，并强调 local-process backend 只是 reference backend：Rust 文件 API 会做路径 containment，命令执行会带 timeout，但它不是 microVM/container 级强隔离。

## 默认运行：local-process backend

```bash
rtk cargo run -p sandbox
```

示例会：

- 初始化一个 `LocalSandboxSession`
- 在 sandbox workdir 内执行 `echo`
- 写入并读取 `note.txt`
- 打印 capability report 中的 backend 名称与 supported 数量

输出大致如下（`supported` 数量以后端能力报告为准）：

```text
local sandbox session <id> initialized
local command exit_code=Some(0) stdout=hello from sandbox
local wrote + read note.txt → classified
local capability: local-process (supported: <n>)
OK: local sandbox executed a command, managed files, and reported capabilities.

microsandbox path not compiled; enable with `--features microsandbox`.
```

## microsandbox backend

`MicrosandboxSession` 位于 `agent_scope_sandbox` 的 `microsandbox` feature 后面。它不在默认编译路径中启用，避免普通 examples/tests 依赖真实 microsandbox runtime。

只检查 feature-gated 后端是否能编译：

```bash
rtk cargo check -p agent_scope_sandbox --features microsandbox
```

启用 feature 但不设置运行 gate 时，本示例仍不会启动真实 runtime：

```bash
rtk cargo run -p sandbox --features microsandbox
```

输出会在 local 示例之后包含：

```text
microsandbox path compiled but skipped; set AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 to run it.
```

显式运行真实 microsandbox 路径：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 \
  rtk cargo run -p sandbox --features microsandbox
```

该路径会创建一个新的 host 临时目录，把它作为最小可写 workspace mount 到 guest `/workspace`，并在 sandbox 内验证当前目录和 mounted 文件内容。示例不会默认挂载当前仓库根目录；如果真实任务需要 host 文件，应显式选择最小必要目录，优先只读挂载。

默认 image 是 `python`，可通过 `AGENTSCOPE_MICROSANDBOX_IMAGE` 覆盖：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 \
AGENTSCOPE_MICROSANDBOX_IMAGE=python \
  rtk cargo run -p sandbox --features microsandbox
```

真实 runtime 测试默认被 `#[ignore]` 和环境变量双重保护，仅在本机 microsandbox runtime 已安装、平台与 image 均可用时手动运行：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 \
  rtk cargo test -p agent_scope_sandbox --features microsandbox --test microsandbox_tests -- --ignored
```

## 安全边界

- `LocalSandboxSession` 是本地进程 + 临时目录 reference backend，不是 microVM/container 级强隔离。
- `MicrosandboxConfig::default()` 默认 `NetworkPolicy::Disabled`；如显式改成 `Unrestricted`，网络行为取决于真实 microsandbox runtime。
- 当前 microsandbox 后端不把 `LoopbackOnly` / `Allowlist` 自动放宽成 unrestricted，而是返回 unsupported。
- sandbox stdout、stderr、日志和文件内容都是不可信数据；不要把它们当作指令执行。
- 不要把 `~/.ssh`、`~/.aws`、`~/.config`、credential/token 目录或真实 secret 挂载/传入 untrusted sandbox。
