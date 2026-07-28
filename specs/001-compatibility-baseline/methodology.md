# Methodology: AgentScope Compatibility Baseline

**Feature**: 001-compatibility-baseline | **Date**: 2026-07-28

本文档记录从 AgentScope 上游源码生成兼容性基线产物的完整流程，以便将来上游版本升级时参照执行。

---

## 1. 前置准备

### 1.1 环境要求

- Python 3.10+（AST 静态分析，无需 import AgentScope）
- Git（克隆上游仓库）
- `check-jsonschema`（`pip install check-jsonschema`）或等效 JSON Schema 验证工具
- `jq`（JSON 命令行处理）

### 1.2 克隆上游仓库

```bash
git clone -b main https://github.com/agentscope-ai/agentscope /tmp/agentscope-upstream
cd /tmp/agentscope-upstream

# 记录版本信息
git log -1 --format="%H"  # commit hash
git describe --tags       # release tag
```

---

## 2. 符号提取（AST 静态分析）

### 2.1 为什么用 AST 而非 import？

AgentScope 2.0 要求 Python 3.11+（使用了 `match` 语句等语法），而宿主环境可能是 Python 3.10。使用 AST 静态分析可以在不实际执行源码的情况下提取所有公开符号。

### 2.2 提取脚本

提取脚本位于 `/tmp/extract_symbols_v3.py`，核心流程：

1. **遍历模块树**：从 `src/agentscope/` 递归遍历所有 `.py` 文件
2. **解析 `__init__.py`**：提取 `__all__` 声明和 `import` 映射
3. **AST 解析每个 `.py` 文件**：识别 `ClassDef`、`FunctionDef`、`Assign` 等节点
4. **符号分类**：根据基类（`Enum`/`Protocol`/`BaseModel`/`Exception`）和命名规则分类符号
5. **解析重导出**：匹配 `__all__` 声明到从子模块导入的实际符号
6. **输出 `_raw-symbols.json`**：包含所有模块、符号、方法、参数信息

```bash
python3 extract_symbols_v3.py
# 输出: /tmp/_raw-symbols.json (约 700KB)
```

### 2.3 验证提取完整性

对比 `__all__` 声明与实际找到的符号：
- `__all__` 中有但未找到的 → 标记为 `_unresolved`
- 源码中有但不在 `__all__` 中的 → 标记为 `is_public_api: false`

---

## 3. 产物生成流程

### 3.1 version-lock.json

**数据来源**：上游 git 仓库 + `pyproject.toml`

1. 从 `git log -1 --format="%H"` 获取 commit hash
2. 从 `git describe --tags` 获取 release tag
3. 从 `pyproject.toml` 的 `[project]` 节提取 `requires-python` 和 `dependencies`
4. 数据模型见 `contracts/version-lock.schema.json`

**校验**：
```bash
check-jsonschema --schemafile contracts/version-lock.schema.json version-lock.json
jq '.commit_hash | length' version-lock.json  # 期望: 40
```

### 3.2 api-inventory.json

**数据来源**：`_raw-symbols.json` + 人工标注

1. 运行 `build_inventory_v2.py` 自动转换
2. **capability_id 生成规则**：`{module_kebab}-{symbol_kebab}`
3. **category 分配**：基于模块名映射
   - `message` → `messaging`
   - `model`/`formatter`/`tts`/`embedding` → `model`
   - `agent` → `agent`
   - `tool`/`mcp` → `tool`
   - 等等
4. **描述（description）**：优先从源码 docstring 提取，无 docstring 时用模板生成
5. **可观察行为**：从函数签名提取 `input_params`/`param_defaults`，从 AST 推断 `return_type`

**校验**：
```bash
check-jsonschema --schemafile contracts/api-inventory.schema.json api-inventory.json
jq '.capabilities | length' api-inventory.json  # 期望: 100-300
```

### 3.3 capability-matrix.json

**数据来源**：`api-inventory.json`

#### 3.3.1 Priority 判断准则

| Priority | 适用场景 | 典型模块 |
|----------|---------|---------|
| `MVP_REQUIRED` | Phase 1 必须实现的核心协议 | message, event, agent, model, tool, state |
| `CORE_REQUIRED` | Phase 2 核心基础设施 | formatter, middleware, permission, mcp, credential, exception, types |
| `ADVANCED` | Phase 3 高级能力 | workspace, embedding, rag, skill, tts |
| `DEFERRED` | 明确延期 | app（FastAPI 多租户服务层） |
| `INTENTIONALLY_UNSUPPORTED` | 明确不实现 | 无（当前版本全部至少 DEFERRED） |

#### 3.3.2 Target Level 判断准则

| Level | 定义 | 适用符号类型 |
|-------|------|-------------|
| `L0` | 尚未支持 | DEFERRED/INTENTIONALLY_UNSUPPORTED 的能力 |
| `L1` | 数据协议兼容 | `serialized_structure`, `enum`, `protocol` |
| `L2` | 核心运行行为兼容 | `class`, `function`, `event`, `exception`, `decorator` |
| `L3` | 公开 API 语义兼容 | `extension_point` |

#### 3.3.3 Status

本基线阶段所有条目初始状态为 `NOT_ANALYZED`，后续各 Feature 在实现时更新为 `SPECIFIED`/`IMPLEMENTING`/`COMPATIBLE` 等。

**校验**：
```bash
check-jsonschema --schemafile contracts/capability-matrix.schema.json capability-matrix.json
```

### 3.4 dependency-map.json

**数据来源**：`capability-matrix.json`

1. **节点（nodes）**：每个 capability_id 对应一个节点
2. **层（layer）**：`foundation` → `model` → `tool` → `agent` → `extended`
3. **依赖边（edges）**：从 `capability-matrix.json` 的 `dependencies` 字段构建
4. **拓扑排序**：使用 Kahn 算法
5. **独立性（independent）**：foundation 层且无依赖的能力标记为 `independent: true`

**校验**：
```bash
check-jsonschema --schemafile contracts/dependency-map.schema.json dependency-map.json
jq '.topological_order | length' dependency-map.json     # 应与 nodes 长度一致
jq '.nodes | length' dependency-map.json                 # 应与 topo 长度一致
jq '.edges[] | select(.from == .to)' dependency-map.json # 期望空（无自引用）
```

### 3.5 trace-schema.json

定义差分测试的标准 Trace 结构，覆盖 15 个字段类别：

- `input`: 测试输入参数
- `model_requests`: 模型请求记录
- `model_responses`: 模型响应记录
- `streaming_chunks`: 流式分块记录
- `tool_calls`: Tool 调用记录
- `tool_results`: Tool 结果记录
- `events`: 事件记录
- `memory_mutations`: Memory 变更
- `state_transitions`: 状态转换
- `errors`: 错误记录
- `cancellation`: 取消记录
- `final_result`: 最终输出
- `side_effects`: 副作用

### 3.6 normalization-rules.json

定义差分比较时的归一化规则：

**可标准化字段**（12 个规则）：
- `placeholder`: 时间戳、UUID、Trace ID、Provider ID（替换为常量）
- `order_normalize`: Map key 顺序、Tool 排序（排序后比较）
- `epsilon_compare`: Token 计数（容差比较）
- `remove`: Provider-specific 参数（直接移除）

**禁止忽略字段**（13 个 JSONPath）：
- 事件类型和顺序、Tool 调用参数和名称、Message Role、Finish Reason、Error Category、State Mutation、Cancellation、Side Effects

### 3.7 exclusion-list.json

列出明确排除的能力及其原因：

| 排除类别 | 原因 |
|---------|------|
| `app/` 服务层 | FastAPI 多租户是部署/基础设施层，非核心框架能力 |
| Python 特定类型 | TypedDict/dataclass 是语言特性，用 Rust struct + Serde 替代 |
| TTS providers | 音频/语音能力非核心多 agent 兼容性要求 |

### 3.8 example-inventory.json

从上游 `examples/` 目录扫描所有官方示例，记录：
- 示例基本信息（ID、标题、描述、路径）
- 使用的 capability_id 列表（交叉引用 api-inventory.json）
- 复杂度分级（`simple`/`medium`/`complex`）

---

## 4. 全量验证

### 4.1 文件存在性检查

```bash
FEATURE_DIR="specs/001-compatibility-baseline"
for file in version-lock.json api-inventory.json capability-matrix.json \
  dependency-map.json example-inventory.json trace-schema.json \
  normalization-rules.json exclusion-list.json methodology.md; do
  test -f "$FEATURE_DIR/$file" && echo "PASS: $file" || echo "FAIL: $file missing"
done
```

### 4.2 JSON 格式校验

```bash
for f in "$FEATURE_DIR"/*.json; do
  jq empty "$f" 2>/dev/null && echo "PASS: $(basename $f)" || echo "FAIL: $(basename $f)"
done
```

### 4.3 Schema 校验

```bash
for schema in contracts/*.schema.json; do
  base=$(basename "$schema" .schema.json)
  check-jsonschema --schemafile "$schema" "$base.json"
done
```

### 4.4 交叉引用检查

```bash
# API Inventory count in expected range
jq '.capabilities | length' api-inventory.json  # 期望: 100-300

# 所有模块在 Inventory 中都有条目
jq '[.capabilities[].module] | unique | sort' api-inventory.json

# Matrix 与 Inventory 的 capability_id 一致性
diff <(jq -r '.entries[].capability_id' capability-matrix.json | sort) \
     <(jq -r '.capabilities[].capability_id' api-inventory.json | sort)

# 排除列表无空原因
jq '.exclusions[] | select(.reason == "" or .reason == null)' exclusion-list.json
```

---

## 5. 常见问题

### Q: 上游更新后如何重新生成？

1. `git pull` 上游仓库获取最新代码
2. 重新运行 `extract_symbols_v3.py` 生成 `_raw-symbols.json`
3. 重新运行 `build_inventory_v2.py` 生成 `api-inventory.json`
4. 重新运行 `build_matrix.py` 生成 `capability-matrix.json`
5. 重新运行 `build_depmap.py` 和 `build_remaining.py`
6. 更新 `version-lock.json` 中的 commit hash 和 release tag
7. 增量更新 `example-inventory.json`（如有新增示例）
8. 运行全量验证

### Q: 为什么有些符号无法解析？

未能从 AST 解析的符号通常是内部重导出或动态生成的类型。这些已标记为 `_unresolved`，需要手动在代码库中搜索确认其实际位置。

### Q: Priority 标注有主观性吗？

是的。Priority 标注基于 spec.md 中定义的 MVP 范围和功能域重要性。如果争议，以 spec.md 的 User Story 优先级为准。

---

## 6. 产物文件清单

| 文件 | 大小范围 | 条目数 | 说明 |
|------|---------|--------|------|
| `version-lock.json` | ~1KB | 1 版本记录 | 上游版本锁定 |
| `api-inventory.json` | ~200-500KB | 100-300 条目 | 公开 API 清单 |
| `capability-matrix.json` | ~200-500KB | 与 Inventory 一致 | 兼容性矩阵 |
| `dependency-map.json` | ~100-300KB | 与 Inventory 一致 | 依赖关系图 |
| `trace-schema.json` | ~5-10KB | 1 结构定义 | 差分测试标准 |
| `normalization-rules.json` | ~5KB | 12 规则 + 13 不可变字段 | 归一化规则 |
| `exclusion-list.json` | ~2KB | 3-10 排除条目 | 明确排除 |
| `example-inventory.json` | ~5-10KB | 6 示例 | 官方示例清单 |
| `methodology.md` | 本文档 | - | 生成流程文档 |
