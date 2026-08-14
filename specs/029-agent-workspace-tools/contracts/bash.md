# Contract: Bash & PowerShell 命令工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_bash.py` / `_powershell.py`（上游 `9d1026fa`）

## 1. Bash

### 工具名

`Bash`

### 描述（模型面对齐 Python `_bash.py:31`）

执行 shell 命令并返回输出。工作目录在命令间持久，但 shell 状态不持久。**IMPORTANT**: 优先使用专用工具（Glob 用于文件搜索、Grep 用于内容搜索、Read 用于读文件、Edit/Write 用于文件修改），避免用 Bash 运行 `find`/`grep`/`cat`/`head`/`tail`/`sed`/`awk`/`echo`。可选 `timeout`（毫秒，默认 120000，最大 600000）。

### 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The bash command to execute."
    },
    "description": {
      "type": "string",
      "description": "Clear, concise description of what this command does."
    },
    "timeout": {
      "type": "integer",
      "description": "Optional timeout in milliseconds (default: 120000, max: 600000)",
      "default": 120000,
      "maximum": 600000,
      "minimum": 0
    }
  },
  "required": ["command"]
}
```

### 行为契约

| 步骤 | 行为 |
|------|------|
| 工作目录 | 绑定 workspace `workdir`；命令间目录持久、shell 状态不持久 |
| 执行 | 经 `WorkspaceBackend::exec_shell`（argv 或 `/bin/sh -c`），不是直接 spawn |
| 超时 | `timeout` 毫秒钳制到 max 600000；超时终止命令（kill process group），返回 timeout 类别错误 |
| 输出 | stdout+stderr 合并，超过 30000 字符截断加 `... (output truncated)` |
| 退出码 | 非零退出 → `ExecutionFailure`（`command_failed`）；stdout/stderr 附在错误内容中 |

### 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 命令为空 | `ValidationFailure` | `invalid_arguments` |
| 超时 | `Timeout` | `command_timeout` |
| 非零退出 | `ExecutionFailure` | `command_failed` |
| 路径逃逸（cwd 越界） | `PermissionDenied` | `path_outside_workspace` |

### 只读判定

`is_read_only = false`（静态）。按调用判断只读命令（如 `ls`/`git status`）在 permission 层处理；含命令替换/进程替换等动态结构视为不可静态分析，不判只读。

## 2. PowerShell

### 工具名

`PowerShell`

### 描述（对齐 Python `_powershell.py:29`）

执行 PowerShell 命令并返回输出。每个命令从配置的工作目录开始，但 PowerShell 会话状态不持久。优先使用专用工具（Glob 而非 Get-ChildItem、Grep 而非 Select-String、Read 而非 Get-Content、Edit/Write 而非 Set-Content/Out-File）。

### 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The PowerShell command to execute."
    },
    "description": {
      "type": "string",
      "description": "Clear, concise description of what this command does."
    },
    "timeout": {
      "type": "integer",
      "description": "Optional timeout in milliseconds (default: 120000, max: 600000)",
      "default": 120000,
      "maximum": 600000,
      "minimum": 0
    }
  },
  "required": ["command"]
}
```

### 可用性（FR-017）

- **仅 Windows 环境启用**（`ToolAvailability.requires_windows_shell = true`）。
- 非 Windows 平台：不注入默认工具集；显式请求时返回 `UnsupportedCapability`（`unsupported_capability`）。
- 可执行文件探测：`pwsh` → `powershell.exe`（对齐 Python `_SHELL_CANDIDATES`）。

### 行为契约

| 步骤 | 行为 |
|------|------|
| 编码 | 命令 UTF-16LE base64 编码，经 `-EncodedCommand` 执行（对齐 Python `_powershell.py:189`） |
| 超时 | 同 Bash（默认 120000ms，最大 600000ms），超时 kill process group |
| 输出 | stdout+stderr 合并，30000 字符截断 |
| 权限 | 每次调用需确认（无安全分类，对齐 Python `_powershell.py:151`） |

### 错误契约

同 Bash，额外：
| 非 Windows 显式请求 | `UnsupportedCapability` | `unsupported_capability` |
| 可执行文件缺失 | `ExecutionFailure` | `command_failed` |

## 依赖方向

`agent_scope_tool::builtin` → `agent_scope_workspace::WorkspaceBackend`（`exec_shell`）。不反向依赖。

## 交叉引用

- Python: `_bash.py`、`_powershell.py`
- 既有: `pi-rust/src/tools.rs` 的 `bash_tool`（`is_destructive_command` 分类器可复用）
- Spec: FR-003~005, FR-017~018
