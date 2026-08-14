# Research: Agent Workspace Built-in Tools

## Decision 1: Register workspace built-ins through `agent_scope_tool` adapters backed by `WorkspaceBase::get_backend()`

**Decision**: Implement executable built-in tools in `agent_scope_tool` as `Tool` implementations that hold a workspace backend handle or a thin workspace tool context. Keep `agent_scope_workspace` responsible for lightweight `ToolInfo` discovery and workspace containment.

**Rationale**: Existing crate direction already allows `agent_scope_tool` to depend on `agent_scope_workspace`, while `agent_scope_workspace` intentionally does not depend on `agent_scope_tool`. This preserves Constitution Article 11 and avoids a cycle. `WorkspaceBase::list_tools()` can continue returning machine-readable metadata, while `ToolKit` receives actual executable adapters only when an agent has workspace access.

**Alternatives considered**:
- Put concrete tool implementations in `agent_scope_workspace`: rejected because it would require workspace to depend on the tool trait crate or duplicate execution contracts.
- Put workspace traits in `agent_scope_tool`: rejected because it would invert the existing dependency boundary and contaminate the tool abstraction with workspace implementation concerns.

## Decision 2: Inject tools only from agent configuration when workspace is explicitly present

**Decision**: Extend the agent construction path to merge workspace built-ins into the agent `ToolKit` only when the agent configuration carries an initialized workspace reference.

**Rationale**: FR-001 and FR-002 require an explicit capability boundary. Existing `ReActAgent` construction already centralizes built-in task tool registration, so workspace tool registration should follow the same opt-in pattern instead of making `ToolKit::new()` globally register file or shell tools.

**Alternatives considered**:
- Auto-register workspace tools in every `ToolKit::new()`: rejected because agents without workspace access would expose file mutation and command tools.
- Require callers to register every workspace tool manually: rejected because it defeats the feature's default-tool objective.

## Decision 3: Use per-tool-session read state for guarded `Edit` and overwrite `Write`

**Decision**: Introduce a shared `WorkspaceToolSession` containing a read-file set keyed by normalized workspace paths. `Read` records successful reads; `Edit` and `Write` require the target path to be in that set before modifying an existing file.

**Rationale**: FR-008 and FR-012 are session-scoped safety constraints, not global filesystem properties. A shared session object lets tools remain independent `Tool` implementations while enforcing the same read-before-modify invariant across calls.

**Alternatives considered**:
- Store read state inside individual tools: rejected because `Read` and mutation tools must coordinate.
- Store read state in `WorkspaceBase`: rejected because read-before-modify is an agent tool-session policy and should not alter the generic workspace abstraction.

## Decision 4: Name and parameter compatibility should follow the feature spec, not current placeholder `list_tools()` metadata

**Decision**: Public tool contracts use names and parameters from the spec: `Bash.command`, `Bash.description`, `Bash.timeout`; `Edit.file_path`, `Edit.old_string`, `Edit.new_string`, `Edit.replace_all`; `Write.file_path`, `Write.content`; `Grep.pattern` and search controls; `Glob.pattern` and optional root; `Skill.skill`; `ResetTools` group reset fields.

**Rationale**: Current `LocalWorkspace::list_tools()` metadata is lightweight and uses earlier placeholder names such as `path`, `old`, and `new`. FR-004 through FR-022 define the stable public-facing contract for this feature and must become the implementation target.

**Alternatives considered**:
- Keep existing `path`/`old`/`new` names for `Edit`: rejected because it conflicts with the approved feature requirements.

## Decision 5: `PowerShell` is conditionally available

**Decision**: Expose `PowerShell` only when the runtime environment supports Windows shell execution. On non-Windows platforms it may be absent from auto-injected tools; if a caller requests it explicitly, return `UnsupportedFeature` or equivalent typed unsupported-capability error.

**Rationale**: FR-017 says PowerShell is environment-dependent. Making it unconditional on macOS/Linux would create a tool that cannot reliably execute and would violate actionable error expectations.

**Alternatives considered**:
- Always register `PowerShell` and fail at call time: rejected for default tool-list clarity on non-Windows environments.

## Decision 6: Search tools should be native Rust implementations over workspace backend APIs

**Decision**: Implement `Glob` and `Grep` using workspace `list_dir`/`read_file` traversal plus Rust matching crates or standard matching logic, with explicit max-result limits and output modes.

**Rationale**: The spec prioritizes dedicated search tools over shell commands. Native implementation preserves workspace containment, produces structured bounded output, and avoids platform-specific shell differences.

**Alternatives considered**:
- Shell out to `grep`/`rg`/`find`: rejected because those tools may be unavailable, produce platform-specific output, and bypass structured result contracts.

## Decision 7: Tool invocation trace uses existing agent tool-call event path plus parameter summaries

**Decision**: Preserve current ReAct tool-call event ordering and add/verify trace metadata for workspace built-ins: tool name, summarized arguments, success/error category, and ordering relative to agent events.

**Rationale**: Constitution Article 7 makes Trace a core acceptance artifact. FR-025 and SC-006 require observability without changing execution order.

**Alternatives considered**:
- Add separate workspace-only tracing outside the agent loop: rejected because it risks divergent ordering and duplicated trace semantics.

---

## Python 参考实现契约事实（vendored 源码，上游 commit `9d1026fa`）

**来源**: `agentscope/src/agentscope/tool/_builtin/`（项目内 vendored Python 参考实现）。以下契约是本 feature 的兼容基准（宪法 Art.1/Art.3）。

### 内置工具清单与来源文件

| 工具 | Python 类 | 源文件 | 是否 MCP | is_read_only | is_concurrency_safe | is_state_injected |
|------|-----------|--------|----------|--------------|---------------------|-------------------|
| Bash | `Bash` | `_bash.py:25` | 否 | 否（`check_read_only` 按调用判断） | 否 | 否 |
| Read | `Read` | `_read.py:19` | 否 | 是 | 是 | 是 |
| Edit | `Edit` | `_edit.py:25` | 否 | 否 | 否 | 是 |
| Write | `Write` | `_write.py:26` | 否 | 否 | 否 | 是 |
| Grep | `Grep` | `_grep.py:39` | 否 | 是 | 是 | 否 |
| Glob | `Glob` | `_glob.py:42` | 否 | 是 | 是 | 否 |
| PowerShell | `PowerShell` | `_powershell.py:23` | 否 | 否 | 否 | 否 |
| ResetTools | `ResetTools`（工具名 `reset_tools`） | `_meta.py:21` | 否 | 否 | 是 | 是 |
| Skill | `SkillViewer`（工具名 `Skill`） | `_skill.py:18` | 否 | 是 | 是 | 是 |

### 各工具输入契约（Python `input_schema`，来自实际源码）

**Bash**（`_bash.py:101`）:
- `command` (string, required) — shell 命令
- `description` (string, optional) — 命令说明
- `timeout` (integer, optional, default 120000, max 600000, min 0) — 超时毫秒
- 描述强调：优先使用专用工具（Glob/Grep/Read/Edit/Write），不用 find/grep/cat/sed/awk/echo

**Read**（`_read.py:38`）:
- `file_path` (string, required) — 绝对路径
- `offset` (integer, optional, default 1, min 1) — 起始行（1-based）
- `limit` (integer, optional, default 2000, max 2000, min 1) — 最大行数
- 输出格式：`cat -n` 格式（6 位填充行号 + tab + 内容）
- **关键**: Read 通过 `_agent_state.tool_context.cache_file()` 缓存已读文件（`_read.py:244`），Edit/Write 依赖此缓存做 read-before-modify 守卫

**Edit**（`_edit.py:50`）:
- `file_path` (string, required)
- `old_string` (string, required)
- `new_string` (string, required)
- `replace_all` (boolean, optional, default false)
- **守卫**: 未读文件返回错误 "Error: To edit a file, you must first read it using the Read tool."（`_edit.py:310`）；`old_string` 未找到返回 "Error: old_string not found in {file_path}"；多次出现且未 replace_all 返回错误提示更具体（`_edit.py:350`）

**Write**（`_write.py:43`）:
- `file_path` (string, required)
- `content` (string, required)
- **守卫**: 已存在文件未读返回 "Error: File {file_path} exists but has not been read yet..."（`_write.py:258`）

**Grep**（`_grep.py:58`）:
- `pattern` (string, required) — 正则
- `path` (string, optional) — 搜索路径（默认 cwd）
- `output_mode` (enum: content/files_with_matches/count, default files_with_matches)
- `glob` (string, optional) — 文件过滤
- `type` (string, optional) — rg --type
- `-A`/`-B`/`-C`/`context` (integer, optional) — 上下文行
- `n` (boolean, default true) — 行号
- `i`/`case_insensitive` (boolean, default false) — 大小写
- `multiline` (boolean, default false)
- `head_limit` (integer, default 250, min 0) — 结果上限
- `offset` (integer, default 0, min 0)
- 实现：ripgrep（`rg`），经 `backend.exec_shell` 调用（`_grep.py:289`）
- VCS 目录排除：`.git/.svn/.hg/.bzr/.jj/.sl`（`_grep.py:18`）

**Glob**（`_glob.py:58`）:
- `pattern` (string, required) — glob 模式（如 `**/*.js`）
- `path` (string, optional) — 基础目录（默认 cwd）
- 实现：调用 `_glob_helper.py` 脚本（`_glob.py:256`），结果按 mtime 最新优先排序

**PowerShell**（`_powershell.py:62`）:
- `command` (string, required)
- `description` (string, optional)
- `timeout` (integer, optional, default 120000, max 600000)
- 实现：base64 编码命令经 `-EncodedCommand` 执行（`_powershell.py:189`）
- 可执行文件探测顺序：`pwsh` → `powershell.exe`（`_powershell.py:20`）
- 权限：每次调用 ASK（无安全分类，`_powershell.py:151`）

**ResetTools**（`_meta.py:21`）:
- 工具名：`reset_tools`（Python 端下划线命名；Rust 侧按 spec 命名为 `ResetTools`）
- 输入：动态生成——每个非 "basic" 的 tool group 一个布尔字段（default false）
- 语义：**输入布尔值是工具组的最终激活状态，非增量**（`_meta.py:34`）；未显式置 true 的组全部停用
- `basic` 组始终激活（`_meta.py:66` 跳过）
- 更新 `_agent_state.tool_context.activated_groups`（`_meta.py:103,120`）
- 返回：激活工具组的 instructions 渲染结果

**Skill**（`_skill.py:18`）:
- 工具名：`Skill`（SkillViewer 类）
- 输入：`skill` (string, required) — 精确技能名
- 通过 `get_skills_method(activated_groups)` 取激活组的技能（`_skill.py:112`）
- 未找到返回 `SkillNotFoundError: Skill '{skill}' not found.`（`_skill.py:120`）

### Workspace 工具装配机制（Python）

- `WorkspaceBase.list_tools()`（`workspace/_base.py:362`）返回六个内置工具实例（Bash/Edit/Glob/Grep/Read/Write），每个绑定 `self.get_backend()`，Bash 绑定 `cwd=self.workdir`。
- `LocalWorkspace`（`workspace/_local_workspace.py:131-149`）在 Windows 上以 `PowerShell(cwd=workdir)` 替代 `Bash`（012 spec 记录此行为）。
- 关键差异：**Python 的 `list_tools()` 返回的是可执行的 Tool 实例**，调用方（用户代码）负责把它们合并进 Toolkit；本 feature 要求"workspace 启用后自动注入"，故 Rust 侧需在 agent 构造路径自动合并（增强，非 Python 行为盲抄）。
- 工具激活状态（ResetTools 读写）位于 `AgentState.tool_context.activated_groups: list[str]`；`basic` 组始终激活，其余组由 ResetTools 控制布尔输入。

### 兼容性要点（Rust 侧实现目标）

1. **工具名**：Bash/Read/Edit/Write/Grep/Glob/PowerShell/Skill 与 Python 一致；ResetTools 用 spec 要求的 `ResetTools`（Python 端为 `reset_tools`），在兼容性矩阵记录该偏差。
2. **参数 schema**：上述 JSON Schema 逐字段对齐（参数名、类型、默认值、required、minimum/maximum）。
3. **错误语义**：read-before-modify、old_string 未找到/非唯一、Skill 未找到的错误文本对齐。
4. **read-state**：Python 用 `tool_context.cache_file` 缓存行；Rust 用 `WorkspaceToolSession.read_files`（BTreeSet 归一化路径）达到等价守卫。
5. **激活状态**：复用 `AgentState.tool_context.activated_groups`，与 Python 一致。
6. **命令执行**：经 `WorkspaceBackend::exec_shell`（Rust 已有，含有界输出/超时/kill_on_drop），不直接 spawn。
