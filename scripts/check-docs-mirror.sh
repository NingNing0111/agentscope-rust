#!/usr/bin/env bash
# check-docs-mirror.sh — docs/rust ↔ docs/python 镜像一致性一键校验
#
# 覆盖（对应 quickstart.md 验证场景）：
#   场景 A：docs/rust/zh 与 docs/python/zh 目录树一比一（en 侧 openapi.json 为登记例外）
#   场景 B：每页有「Rust 实现状态」状态块；计划中页无 Rust 代码块
#   场景 E：站内链接（版本化 / 相对路径）无悬空
#
# 用法：
#   scripts/check-docs-mirror.sh            # 从仓库根运行，全量校验
#   scripts/check-docs-mirror.sh --quiet    # 仅输出失败项（CI 友好）
#
# 退出码：0 = 全部通过；1 = 存在失败项

set -u

# 仓库根（脚本位于 scripts/ 下）
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PY_DOCS="$ROOT/docs/python/zh"
RS_DOCS="$ROOT/docs/rust/zh"
MIRROR_MAP="$ROOT/docs/rust/mirror-map.md"

QUIET=0
[ "${1:-}" = "--quiet" ] && QUIET=1

pass=0
fail=0

report() { # report <PASS|FAIL> <消息>
  if [ "$1" = "PASS" ]; then
    pass=$((pass + 1))
    [ "$QUIET" -eq 0 ] && printf '  ✓ %s\n' "$2"
  else
    fail=$((fail + 1))
    printf '  ✗ %s\n' "$2"
  fi
  return 0  # 恒成功，避免破坏调用处 && / || 短路
}

section() {
  [ "$QUIET" -eq 0 ] && printf '\n== %s ==\n' "$1"
}

[ "$QUIET" -eq 0 ] && printf 'docs/rust ↔ docs/python 镜像一致性校验\n'

if [ ! -d "$RS_DOCS" ]; then
  printf '✗ 目录缺失：docs/rust/zh 不存在，请在仓库根运行。\n' >&2
  exit 2
fi

# ---- 场景 A：页面数量与可选镜像结构比较 ----
section "场景 A：页面集合"
rs_md_count="$(find "$RS_DOCS" -type f -name '*.md' | wc -l | tr -d ' ')"
rs_mdx_count="$(find "$RS_DOCS" -type f -name '*.mdx' | wc -l | tr -d ' ')"
[ "$rs_md_count" -eq 51 ] && report PASS "Rust 文档包含 51 个 .md 页面" || report FAIL "Rust .md 页面数=${rs_md_count}（预期 51）"
[ "$rs_mdx_count" -eq 0 ] && report PASS "Rust 文档不含 .mdx 页面" || report FAIL "Rust .mdx 页面数=${rs_mdx_count}（预期 0）"

if [ -d "$PY_DOCS" ]; then
  py_files="$(cd "$PY_DOCS" && find . -type f -name '*.mdx' | sed -E 's|^\./||; s|\.mdx$||' | sort)"
  rs_files="$(cd "$RS_DOCS" && find . -type f -name '*.md' | sed -E 's|^\./||; s|\.md$||' | sort)"
  py_only="$(comm -23 <(printf '%s\n' "$py_files") <(printf '%s\n' "$rs_files"))"
  rs_only="$(comm -13 <(printf '%s\n' "$py_files") <(printf '%s\n' "$rs_files"))"
  if [ -z "$py_only" ] && [ -z "$rs_only" ]; then
    count="$(printf '%s\n' "$rs_files" | grep -c .)"
    report PASS "Python/Rust 目录树一致（$count 页）"
  else
    [ -n "$py_only" ] && printf '%s\n' "  缺页（Python 有而 Rust 无）：" && printf '    %s\n' "$py_only"
    [ -n "$rs_only" ] && printf '%s\n' "  多页（Rust 有而 Python 无）：" && printf '    %s\n' "$rs_only"
    report FAIL "Python/Rust 目录树存在差异"
  fi
else
  printf '  SKIP: docs/python/zh 不存在，跳过 Python/Rust 目录树比较；继续 Rust 自洽检查。\n'
fi

mm_count="$(grep -cE '^\| [^|]+ \| [^|]+ \| (已实现|部分支持|计划中) ' "$MIRROR_MAP" 2>/dev/null || true)"
if [ "$mm_count" -eq 51 ]; then
  report PASS "mirror-map 覆盖 51 页登记"
else
  report FAIL "mirror-map 登记数=${mm_count}（预期 51）"
fi

# ---- 场景 B：状态块与无伪兼容 ----
section "场景 B：状态块 + 计划中页无 Rust"

# 收集全部页面（zh 下所有深度）
all_pages="$(cd "$RS_DOCS" && find . -name "*.md" | sort)"

missing_status=0
rust_in_planned=0
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  f="$RS_DOCS/${rel#./}"
  if ! grep -q 'Rust 实现状态' "$f"; then
    printf '  缺状态块: %s\n' "$rel"
    missing_status=$((missing_status + 1))
  fi
  first_status="$(grep -m1 'Rust 实现状态' "$f" | grep -o '已实现\|部分支持\|计划中' | head -1)"
  if [ "$first_status" = "计划中" ] && grep -qE '^```rust' "$f"; then
    printf '  计划中页含 Rust 代码: %s\n' "$rel"
    rust_in_planned=$((rust_in_planned + 1))
  fi
done <<< "$all_pages"
[ "$missing_status" -eq 0 ] && report PASS "全部页面含状态块" || report FAIL "$missing_status 页缺状态块"
[ "$rust_in_planned" -eq 0 ] && report PASS "计划中页无 Rust 代码块" || report FAIL "$rust_in_planned 个计划中页含 Rust 代码"

# ---- 场景 E：站内链接无悬空 ----
section "场景 E：站内链接悬空检测"

# 收集 (源文件, 链接) 对，交给 python3 解析 + 校验
links_tmp="$(mktemp)"
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  f="$RS_DOCS/${rel#./}"
  # 版本化站内链接 /versions/<ver>/zh/<path>（保留完整链接，由 python 判定前缀）
  grep -oE '/versions/[0-9a-z.-]+/zh/[a-zA-Z0-9_./-]+' "$f" | sort -u | while read -r link; do
    printf '%s\t%s\n' "$f" "$link"
  done
  # 相对 markdown 链接 ](path)，仅 ./ 与 ../ 开头
  grep -oE '\]\([.][^)]*\)' "$f" | sed -E 's/^\]\(//; s/\)$//' | sort -u | while read -r link; do
    printf '%s\t%s\n' "$f" "$link"
  done
done <<< "$all_pages" > "$links_tmp"

dead_count=0
if command -v python3 >/dev/null 2>&1; then
  while IFS=$'\t' read -r src link; do
    target="$(RS_DOCS="$RS_DOCS" python3 - "$src" "$link" <<'PY'
import os, sys
src, link = sys.argv[1], sys.argv[2]
rs_docs = os.environ['RS_DOCS']
# 去掉锚点
link = link.split('#', 1)[0]
# 版本化链接目标直接落在 zh/ 下
if link.startswith('/versions/'):
    rel = link.split('/zh/', 1)[1]
    t = os.path.join(rs_docs, rel)
else:
    t = os.path.normpath(os.path.join(os.path.dirname(src), link))
# 目录目标（如指向 examples/<name>/ 的示例链接）
if os.path.isdir(t):
    print(os.path.normpath(t))
    sys.exit(0)
# 无扩展名：解析为 .md
if not os.path.splitext(t)[1]:
    t = t + '.md'
print(os.path.normpath(t))
PY
)"
    if [ ! -e "$target" ]; then
      printf '  悬空链接: %s -> %s\n' "${src#"$ROOT"/}" "$link"
      dead_count=$((dead_count + 1))
    fi
  done < "$links_tmp"
else
  printf '  ⚠ python3 不可用，跳过链接悬空检测\n'
  report FAIL "链接检测依赖 python3，当前不可用"
fi
rm -f "$links_tmp"
[ "$dead_count" -eq 0 ] && report PASS "站内链接无悬空" || report FAIL "$dead_count 个悬空链接"

# ---- 汇总 ----
section "汇总"
if [ "$fail" -gt 0 ]; then
  [ "$QUIET" -eq 0 ] && printf '  通过 %d 项，失败 %d 项\n' "$pass" "$fail"
  exit 1
fi
[ "$QUIET" -eq 0 ] && printf '  全部 %d 项校验通过 ✓\n' "$pass"
exit 0
