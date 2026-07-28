# Quickstart: AgentScope Compatibility Baseline

**Feature**: 001-compatibility-baseline | **Date**: 2026-07-28

本文档描述如何验证兼容性基线的正确性和完整性。

## 前置条件

- 能够访问 AgentScope Python 包的 GitHub 仓库
- Python 3.10+ 环境（仅用于验证符号提取）
- `pip install agentscope`（可安装 AgentScope Python 包）
- `jq` 命令行工具（用于 JSON schema 验证）

## 验证步骤

### 1. 验证版本锁定文件

```bash
# 确认 commit hash 长度正确
jq '.commit_hash | length' specs/001-compatibility-baseline/version-lock.json
# 期望输出: 40

# 验证 schema 合规
# （需要 ajv-cli 或手动对照 contracts/version-lock.schema.json 检查）
```

### 2. 验证 API Inventory 覆盖率

```bash
# 启动 Python 交互式环境，获取 AgentScope 的实际导出列表
python3 -c "
import agentscope
# 列出 agentscope 顶层 __all__ 或 dir() 中的公开符号
public_symbols = [s for s in dir(agentscope) if not s.startswith('_')]
print('Top-level public symbols:', len(public_symbols))
for s in sorted(public_symbols):
    print(f'  {s}')
"

# 检查 api-inventory.json 中条目数是否合理
jq '.capabilities | length' specs/001-compatibility-baseline/api-inventory.json
# 期望: 100-300

# 交叉检查：确保至少每个顶层模块在 Inventory 中都有对应条目
jq '.capabilities[] | select(.symbol_type == "module") | .symbol_name' \
  specs/001-compatibility-baseline/api-inventory.json
```

### 3. 验证能力矩阵一致性

```bash
# 确保 capability-matrix.json 中的每个 capability_id 都能在 api-inventory.json 中找到
jq -r '.entries[].capability_id' specs/001-compatibility-baseline/capability-matrix.json | sort > /tmp/matrix_ids.txt
jq -r '.capabilities[].capability_id' specs/001-compatibility-baseline/api-inventory.json | sort > /tmp/inventory_ids.txt
diff /tmp/matrix_ids.txt /tmp/inventory_ids.txt
# 期望: 所有 ID 一致（无差异）

# 检查所有 priority 和 target_level 值是否合法
jq '.entries[] | select(.priority | IN("MVP_REQUIRED","CORE_REQUIRED","ADVANCED","DEFERRED","INTENTIONALLY_UNSUPPORTED") | not)' \
  specs/001-compatibility-baseline/capability-matrix.json
# 期望: 空（无非法值）
```

### 4. 验证依赖图

```bash
# 确认拓扑排序不包含循环依赖
jq '.topological_order | length' specs/001-compatibility-baseline/dependency-map.json
jq '.nodes | length' specs/001-compatibility-baseline/dependency-map.json
# 期望: 两个长度相等（每个节点都在排序中出现）

# 检查无自引用边
jq '.edges[] | select(.from == .to)' specs/001-compatibility-baseline/dependency-map.json
# 期望: 空
```

### 5. 验证示例清单

```bash
# 确保每个示例至少引用 1 个 capability
jq '.examples[] | select(.capabilities_used | length == 0)' \
  specs/001-compatibility-baseline/example-inventory.json
# 期望: 空
```

### 6. 验证排除清单

```bash
# 确保每个排除条目都有原因说明
jq '.exclusions[] | select(.reason == "" or .reason == null)' \
  specs/001-compatibility-baseline/exclusion-list.json
# 期望: 空
```

### 7. 验证方法论文档

```bash
# 确认 methodology.md 存在且非空
test -s specs/001-compatibility-baseline/methodology.md && echo "PASS" || echo "FAIL"
```

## 自动化验证脚本

以下脚本可一次性运行所有基础验证：

```bash
#!/bin/bash
set -e
FEATURE_DIR="specs/001-compatibility-baseline"

echo "=== Validating AgentScope Compatibility Baseline ==="

# 1. Check all deliverables exist
for file in \
  version-lock.json api-inventory.json capability-matrix.json \
  dependency-map.json example-inventory.json trace-schema.json \
  normalization-rules.json exclusion-list.json methodology.md
do
  test -f "$FEATURE_DIR/$file" && echo "PASS: $file exists" || echo "FAIL: $file missing"
done

# 2. Validate JSON files are well-formed
for f in "$FEATURE_DIR"/*.json; do
  jq empty "$f" 2>/dev/null && echo "PASS: $(basename $f) is valid JSON" \
    || echo "FAIL: $(basename $f) is not valid JSON"
done

# 3. Cross-reference checks
CAP_COUNT=$(jq '.capabilities | length' "$FEATURE_DIR/api-inventory.json")
echo "INFO: API Inventory has $CAP_COUNT capabilities"
test "$CAP_COUNT" -ge 50 && echo "PASS: Capability count >= 50" \
  || echo "WARN: Capability count < 50"

echo "=== Validation complete ==="
```

## 成功标准检查清单

对照 spec.md 中的 SC-001 至 SC-014，手动逐项确认：

- [ ] SC-001: version-lock.json 包含 40 字符 commit hash
- [ ] SC-002: api-inventory.json 覆盖所有顶层公开模块
- [ ] SC-003: 所有能力条目具有完整必填属性
- [ ] SC-004: 所有能力已标记 priority 和 target_level
- [ ] SC-005: MVP_REQUIRED 能力关联源码位置
- [ ] SC-006: MVP_REQUIRED 能力拥有测试场景
- [ ] SC-007: dependency-map.json 可拓扑排序无循环
- [ ] SC-008: MVP_REQUIRED 集合形成可理解子集
- [ ] SC-009: exclusion-list.json 每项附原因说明
- [ ] SC-010: trace-schema.json 覆盖完整生命周期
- [ ] SC-011: normalization-rules.json 区分可标准化/禁止忽略字段
- [ ] SC-012: 所有顶层模块在 Inventory 中有条目或在 Exclusion 中
- [ ] SC-013: 基线中无 Rust 实现细节
- [ ] SC-014: methodology.md 描述生成流程
