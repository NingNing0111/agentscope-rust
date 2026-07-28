# Research: AgentScope Compatibility Baseline

**Feature**: 001-compatibility-baseline | **Date**: 2026-07-28

## Research Tasks & Decisions

### 1. AgentScope Upstream Repository

**Decision**: 目标上游仓库为 `https://github.com/agentscope-ai/agentscope`（AgentScope 2.0，由 Alibaba Tongyi Lab 的 SysML 团队维护）。

**Rationale**: 
- `agentscope-ai/agentscope` 是 AgentScope 2.0 的官方仓库（Stars: ~17,848, License: Apache-2.0）
- 最新稳定 release: **v2.0.4**（另有 v2.0.4.post1）
- 旧仓库 `modelscope/agentscope` 为 1.x 系列（最新 v1.0.18），已不再活跃
- **2.0 是一次完整重写**：旧的 `AgentBase`/`ReActAgent` 继承体系被单一 `Agent` 类 + 配置驱动设计取代
- 文档: https://doc.agentscope.io/
- 示例仓库: `agentscope-ai/agentscope-samples`（按类别和复杂度组织）
- 主仓库 `examples/` 目录包含 7 类基本示例
- Python 版本要求: 3.11+（AgentScope 2.0）

### 2. Symbol Extraction Approach

**Decision**: 使用 Python 标准库 `inspect` + `ast` + `importlib` 编写符号提取脚本，自动化提取模块/类/函数/方法符号，语义信息（描述、优先级、兼容等级、依赖关系）由人工标注补充。

**Rationale**: 
- 纯手动不可靠且不可复现（几百个 API 符号手写容易出错）
- 完全自动化生成代码超出本 Feature 的非目标范围
- 脚本辅助 + 人工标注在准确性和投入之间取得平衡
- Python 标准库即可完成，无需额外依赖

**Alternatives considered**:
- 纯手动阅读源码（规模不可行，100-300 条目）
- 静态分析工具（`pydoc`、`sphinx-autodoc` 等）（提取粒度不够精细，仍需二次处理）
- AST 解析（复杂度过高，`inspect` 模块足够）

**Extraction script outline** (not a deliverable of this feature):
```python
import inspect
import importlib
import agentscope

# 遍历顶层模块
for name in dir(agentscope):
    if name.startswith('_'):
        continue
    obj = getattr(agentscope, name)
    if inspect.ismodule(obj):
        # 记录模块信息
        pass
    elif inspect.isclass(obj):
        # 记录类信息（含方法）
        pass
    elif inspect.isfunction(obj):
        # 记录函数信息
        pass
```

### 3. Static Analysis Scope

**Decision**: 本次基线仅通过源码静态分析和文档分析生成，不实际运行 Python AgentScope。运行时行为验证属于后续各模块 Feature 的范围。

**Rationale**:
- 本 Feature 的目的是建立清单和范围，而非验证行为
- 安装运行 Python AgentScope 会大幅增加 Feature 复杂性
- 宪法第六条和第七条要求的行为级别测试由后续 Feature 各自负责
- 本基线定义的 Trace Schema 和 Normalization Rules 为将来的运行时验证提供标准

**Alternatives considered**:
- 最小运行验证（import 检查）（仍需要安装 Python AgentScope 运行环境，增加复杂度）
- 选取代表性流程记录 Trace（超出静态分析的 scope，应属于后续 Feature）

### 4. Baseline Reproducibility

**Decision**: 基线数据为针对锁定版本的一次性具体产物，同时输出一份 `methodology.md` 描述完整生成流程（包括脚本使用方式、人工判断准则、输出格式说明）。后续上游版本升级时参照此文档重新执行。

**Rationale**:
- 本 Feature 不设计自动化流水线（超出非目标范围）
- 方法文档确保将来团队不从头开始
- 一次性快照固定了一个可验证的参照点

**Alternatives considered**:
- 仅一次性快照（缺乏复现指引，后续升级从零开始）
- 可执行流水线（需要 CI 集成、Python 环境管理，超出本 Feature scope）

### 5. Artifact Organization

**Decision**: 每种基线产物一个独立 JSON 文件，统一放在 `specs/001-compatibility-baseline/` 目录下。

**Rationale**:
- JSON 格式便于自动化工具解析和 CI 集成
- 分文件而非单文件使每个产物可独立引用、独立版本控制 diff
- 与 spec 同目录使基线数据与 spec 紧密关联

**Alternatives considered**:
- 单文件 JSON（一个 100-300 条目的文件难以 diff 和协作）
- Markdown 为主（可读性好但不适合机器解析，不符合宪法对机器可读性的要求）

### 6. Scale Expectation

**Decision**: API Inventory 预期包含 100-300 个能力条目，覆盖 AgentScope 主要公开模块的类、方法、函数和数据结构。

**Rationale**:
- AgentScope 作为 LLM agent 框架，公开 API 规模预期在中等范围
- 核心类型（Message、Model、Tool、Agent、Memory、Event 等）每个估计 10-30 个公开符号
- 5-10 个顶层模块 × 平均 20 个公开符号 = 100-200 条

**Risk**: 若实际分析发现远超此范围，在进度报告中调整预期。

### 7. Known Unknowns

以下问题需要在执行阶段（/speckit-tasks）通过实际分析 AgentScope 源码来回答：

1. AgentScope 2.0 的具体 commit hash（需 clone 仓库后获取）
2. 每个模块的精确公开符号数量（验证 100-300 预期）
3. 官方示例的确切数量和复杂度
4. 是否存在隐含的公开 API（未在 `__init__.py` 导出但可被用户直接 import）
5. 依赖库（Pydantic、httpx、openai 等）的具体版本号

### 8. Confirmed Module Structure (from web research)

AgentScope 2.0 源码位于 `src/agentscope/` 下，核心库模块如下：

| 模块 | 用途 |
|------|------|
| `agent/` | 单一 `Agent` 类 + 配置驱动 ReAct 循环 |
| `model/` | 多 Provider LLM 抽象层（9 种 ChatModel） |
| `message/` | `Msg` 和 `ContentBlock` 数据结构 |
| `event/` | 流式事件系统（`EventType` 枚举 ~20 种事件, `AgentEvent`） |
| `tool/` | Toolkit 系统、工具注册、MCP 集成 |
| `formatter/` | `Msg[]` → Provider 特定 API 格式转换 |
| `middleware/` | 可插拔 Middleware（6 个 hook 点） |
| `permission/` | 细粒度工具执行权限引擎 |
| `workspace/` | 沙箱/运行时抽象（Local、Docker、E2B、K8s、Daytona） |
| `state/` | 可持久化 `AgentState`、Tool Context、Tasks |
| `mcp/` | MCP 协议客户端 |
| `skill/` | Skill 加载 |
| `embedding/` | 文本向量化 |
| `credential/` | API 凭证管理 |

服务层另有 `src/agentscope/app/`（FastAPI 多租户服务）。

**预计约 14 个核心模块**，每模块估计 10-20 个公开符号，总体在 140-280 范围内，符合 100-300 预期。

## Technology Stack Summary

| 用途 | 工具/技术 | 理由 |
|------|----------|------|
| 符号提取 | Python `inspect`, `importlib` | 标准库，无需额外依赖 |
| 版本管理 | `git`, `pip show agentscope` | 获取精确版本信息 |
| 数据存储 | JSON (Draft 2020-12 Schema) | 机器可读，语言无关 |
| 文档 | Markdown | 开发者友好 |
| 验证 | `jq`, JSON Schema validators | 通用工具，CI 友好 |
