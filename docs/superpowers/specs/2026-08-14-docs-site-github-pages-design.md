# AgentScope Rust 中文文档站与 GitHub Pages 发布设计

**日期：** 2026-08-14  
**状态：** 已批准  
**范围：** `docs/rust/zh/` 中文文档迁移、VitePress 站点和 GitHub Pages CI/CD

## 1. 背景

AgentScope Rust 在 `docs/rust/zh/` 中维护中文文档，并通过 `docs/rust/mirror-map.md` 跟踪与 Python 文档的页面镜像关系及 Rust 实现状态。当前中文文档包含 50 个 Mintlify 风格 `.mdx` 页面，使用 `<Note>`、`<Tip>`、`<Card>`、`<CardGroup>`、`<Badge>`、`<Accordion>` 和 `<AccordionGroup>` 等组件。

VitePress 1.6.4 默认发现和构建 `.md` 页面，不把现有 `.mdx` 文件当作页面。因此，本项目不能直接把 50 个 `.mdx` 文件交给 VitePress；必须先完成可审计的原地格式迁移。

仓库目前只有 Rust CI（`.github/workflows/ci.yml`），没有 Node.js 依赖清单、静态站点生成器、GitHub Pages workflow 或自动文档检查。GitHub Pages 尚未启用。

## 2. 已确认决策

1. 仅发布 AgentScope Rust 中文文档。
2. 接受引入 Node.js/npm。
3. 使用产品型首页。
4. 使用 VitePress，并为现有 Mintlify 组件提供轻量 Vue 兼容组件。
5. Pull Request 只检查和构建；`master` push 与手动触发可部署。

## 3. 目标与非目标

### 3.1 目标

1. 将 `docs/rust/zh/` 的 50 个正式页面从 `.mdx` 原地迁移为 VitePress 可发现的 `.md`。
2. 保持 `docs/rust/mirror-map.md`、文件系统和侧边栏三者页面集合一致。
3. 提供产品型首页、完整导航、本地搜索、暗色模式和移动端布局。
4. 兼容文档实际使用的 Mintlify 组件，不要求把所有组件降级成纯 Markdown。
5. 统一站内路由策略，并修复内容根之外的源码、示例和维护文件链接。
6. 在 PR 中自动检查全部页面并执行生产构建。
7. 在 `master` 更新后通过 GitHub 官方 Pages actions 自动发布。
8. 确保 Python 文档、维护元数据和内部设计资料不进入站点产物。
9. 在根 `README.md` 中增加中文文档站入口。

### 3.2 非目标

- 不发布英文站点或 `docs/python/`。
- 不公开 `docs/rust/README.md`、`STATUS-BLOCK.md`、`mirror-map.md` 或 `docs/superpowers/`。
- 不自动生成 Rust API reference。
- 不引入后端、评论、遥测或外部搜索服务。
- 不实现通用 MDX 编译器或运行时 JSX 支持。
- 不修改 Rust 公共 API 或运行时行为。
- 不对与站点构建、链接正确性和读者可执行性无关的内容做无边界重写。

## 4. 技术方案

采用 **VitePress 1.6.4 + Vue 3 兼容组件 + 原地 `.mdx` → `.md` 迁移**。

### 4.1 固定工具版本

- Node.js：22（`.nvmrc` 固定 `22`，GitHub Actions 使用同一主版本）
- VitePress：`1.6.4`，在 `package.json` 中使用精确版本
- npm：随 Node.js 22 提供，最终精确依赖树由提交的 `package-lock.json` 固定

升级 Node.js 或 VitePress 是后续独立维护事项，不在首次上线时使用浮动 `latest`。

### 4.2 为什么原地改名

VitePress 页面发现以 `.md` 为准。与临时复制树或自定义路由插件相比，原地改名具有以下优势：

- 源文件就是发布文件，不产生双份内容；
- Git 可以识别大多数 rename，历史仍可追踪；
- 相对链接、mirror map 和检查器只有一个事实来源；
- 不依赖未验证的 VitePress 内部插件接口；
- 本地编辑器和 CI 的行为一致。

迁移完成后，`docs/rust/zh/**/*.mdx` 数量必须为零，正式页面 `.md` 数量必须为 50。

### 4.3 分阶段门禁

实施不能一次性盲改 50 页，必须按以下顺序：

1. **技术原型：** 迁移首页、FAQ、change log 和一个深层 CardGroup 页面，验证组件、Vue 属性、路由、SSR 和 build。
2. **全量文件迁移：** 原地改名其余页面并同步链接和 mirror map。
3. **检查器：** 建立页面集合、组件、路由和公开边界检查。
4. **首页与导航：** 配置全量 sidebar/nav，完善主题和无障碍。
5. **PR build workflow：** 先验证不部署。
6. **Pages 前置设置与部署：** 启用 Pages 后验证生产发布。
7. **独立审查与验收。**

阶段 1 未通过时不得执行全量迁移。

## 5. 内容与目录架构

VitePress 内容根目录为：

```text
docs/rust/zh/
```

迁移后的核心结构：

```text
docs/rust/zh/
├── index.md
├── quickstart.md
├── release-notes.md
├── building-blocks/**/*.md
├── deploy/**/*.md
├── others/**/*.md
└── .vitepress/
    ├── config.mts
    └── theme/
        ├── index.ts
        ├── custom.css
        └── components/
            ├── Note.vue
            ├── Tip.vue
            ├── Card.vue
            ├── CardGroup.vue
            ├── Badge.vue
            ├── Accordion.vue
            └── AccordionGroup.vue
```

公开页面集合以更新后的 `docs/rust/mirror-map.md` 列出的 50 个 Rust `.md` 页面为权威基线。

以下内容不得复制到构建产物：

```text
docs/python/**
docs/rust/README.md
docs/rust/STATUS-BLOCK.md
docs/rust/mirror-map.md
docs/superpowers/**
```

## 6. Mintlify 组件迁移

### 6.1 组件矩阵

| 现有标签 | VitePress/Vue 映射 | 实际属性 | 语义与要求 |
|---|---|---|---|
| `Note` | `Note.vue` | 无 | 使用提示容器语义，颜色在明暗主题下均满足可读性 |
| `Tip` | `Tip.vue` | 无 | 与 Note 视觉区分，但不只依赖颜色传达类型 |
| `Card` | `Card.vue` | `title`、`icon`、`href`、`cta` | 内链使用 VitePress 路由；外链有可识别行为；整卡不制造嵌套交互冲突 |
| `CardGroup` | `CardGroup.vue` | `cols` | 响应式 CSS Grid；窄屏降为单列 |
| `Badge` | `Badge.vue` | `color`、`size` | 行内状态文本，保持文字标签，不能只显示颜色 |
| `Accordion` | `Accordion.vue` | `title` | 使用 button 控制内容；支持 Enter/Space；维护 `aria-expanded` 和关联 id |
| `AccordionGroup` | `AccordionGroup.vue` | 无 | 组织 Accordion，不改变键盘可达性 |

### 6.2 Vue 模板语法

现有 JSX 风格属性：

```text
cols={2}
```

必须迁移为 Vue Markdown 可接受的绑定：

```text
:cols="2"
```

检查器应禁止公开页面残留 `cols={数字}`。

组件属性采用 fail-closed 策略：组件扫描器发现未登记组件，或关键组件出现未登记属性时，`docs:check` 失败。不得静默吞掉可能改变链接、布局或状态语义的属性。

扫描必须忽略 fenced code block 和 inline code 中看似组件或 Rust 泛型的文本，避免把 `Vec`、`Msg`、`ContentBlock` 等代码标识误判为页面组件。

## 7. 路由与链接政策

### 7.1 站内路由

当前 `/versions/0.1.0/zh/...` 是旧 Mintlify 路由政策。新站内容统一使用 VitePress 根路由，例如：

```text
/quickstart
/building-blocks/agent/overview
```

VitePress 的 `base` 在生产环境添加 `/agentscope-rust/`。内容文件中不得硬编码部署仓库名前缀。

`docs/rust/mirror-map.md` 和 `docs/rust/README.md` 的版本声明必须同步更新为：

> 文档内容使用站点根路由；部署前缀由 VitePress `base` 注入。

检查器禁止 50 个公开页面以及上述维护文件残留 `/versions/0.1.0/zh/` 路径政策。

### 7.2 文件扩展名同步

以下位置必须从 `.mdx` 同步为 `.md`：

- `mirror-map.md` 的 Rust 页面列；
- `docs/rust/README.md` 的导航；
- 页面间显式带扩展名的链接；
- 检查器 fixture 和 sidebar 配置。

### 7.3 内容根之外的链接

公开页面不得使用相对路径链接到内容根外部，因为这些目标不会成为 VitePress 页面。

处理规则：

1. 指向 `STATUS-BLOCK.md`、`mirror-map.md` 的链接改为页面内简短说明或删除链接；维护文件不公开。
2. 指向 `examples/**`、`crates/**` 或其他仓库源码的链接改为 GitHub URL：

```text
https://github.com/NingNing0111/agentscope-rust/tree/master/<path>
https://github.com/NingNing0111/agentscope-rust/blob/master/<path>
```

文档站跟随 `master` 发布，因此源码链接也跟随 `master`，不固定历史 SHA。
3. 页面间链接使用站点根路径或合法相对页面路径。
4. 外部 URL 保持不变。

检查器分别验证站内路由、禁止的内容根外相对链接，以及 GitHub 源码 URL 的仓库路径是否在本地存在。CI 不依赖网络访问 GitHub 来验证本仓库路径。

### 7.4 旧 URL

仓库此前没有启用 GitHub Pages，因此不存在需要保持的生产站点 URL。首发不提供 `/versions/0.1.0/zh/` 重定向；旧路由直接从源内容移除。

## 8. 首页与导航

### 8.1 产品型首页

`docs/rust/zh/index.md` 在现有能力说明基础上构建产品型首页，包括：

1. Hero：AgentScope Rust、项目定位、快速开始和 GitHub 按钮。
2. 核心能力：Agent/事件、Tool/MCP、Memory/RAG、Workspace/Sandbox、权限/HITL、Skill/SubAgent/任务规划。
3. 文档入口：快速开始、构建模块、部署状态、FAQ 和版本说明。
4. 实现状态：解释「已实现 / 部分支持 / 计划中」，不伪造兼容能力。

首页自动化 smoke test 至少断言项目标题、快速开始链接、GitHub 链接和实现状态说明存在。

### 8.2 顶部导航

```text
首页 | 快速开始 | 构建模块 | 部署与集成 | FAQ | GitHub
```

### 8.3 侧边栏

侧边栏显式维护，覆盖 mirror map 的全部 50 个页面：

1. 开始使用；
2. Agent；
3. Context；
4. Model；
5. Tool；
6. Workspace；
7. Message/Event、Memory、Middleware、Permission、Plan、RAG、Console；
8. 部署与集成；
9. Change log 与 FAQ。

「计划中」页面继续公开，用于说明边界；页面现有状态块保持可见。

### 8.4 页面集合约束

`scripts/docs/check-docs.mjs` 比较：

- mirror map 中列出的 50 个 Rust 页面；
- `docs/rust/zh/**/*.md` 实际页面；
- sidebar 配置页面。

这三个集合必须完全相等。`index.md` 已在 50 页集合内，不使用含糊的首页白名单。VitePress 自身生成的 404 页面不参与内容集合。

## 9. Node.js 工具链与检查器

仓库根增加：

```text
package.json
package-lock.json
.nvmrc
```

固定脚本目录：

```text
scripts/docs/
├── check-docs.mjs
└── check-built-site.mjs
```

npm scripts：

```text
docs:dev           启动开发服务器
docs:check         检查组件、属性、页面集合、路由政策和仓库路径
docs:build         构建生产站点
docs:check-built   检查 dist 的链接、资源前缀和公开边界
docs:preview       预览生产构建
```

构建产物固定为：

```text
docs/rust/zh/.vitepress/dist/
```

`docs:check-built` 必须在 `docs:build` 后运行。

## 10. GitHub Actions

新增：

```text
.github/workflows/docs.yml
```

### 10.1 触发条件

- `pull_request` 指向 `master`：检查和构建，不部署。
- `push` 到 `master`：检查、构建和部署。
- `workflow_dispatch`：手动检查、构建和部署。

路径过滤覆盖：

```text
docs/rust/zh/**
docs/rust/README.md
docs/rust/mirror-map.md
scripts/docs/**
package.json
package-lock.json
.nvmrc
.github/workflows/docs.yml
```

### 10.2 Jobs 与最小权限

Workflow 顶层只授予：

```yaml
permissions:
  contents: read
```

`build` job 继承只读权限：

```text
checkout
setup-node (Node 22 + npm cache)
configure-pages
npm ci
npm run docs:check
npm run docs:build
npm run docs:check-built
upload-pages-artifact
```

`deploy` job 仅在以下条件运行：

```text
github.event_name == 'workflow_dispatch'
或
github.event_name == 'push' && github.ref == 'refs/heads/master'
```

`deploy` job 单独授予：

```yaml
permissions:
  pages: write
  id-token: write
```

并配置：

```yaml
environment:
  name: github-pages
  url: ${{ steps.deployment.outputs.page_url }}
```

Pull Request 的 build job 不持有 Pages 写权限或 OIDC 权限。

### 10.3 并发

使用：

```yaml
concurrency:
  group: pages-${{ github.ref }}
  cancel-in-progress: true
```

新的同分支 workflow 取消旧的未完成 build/deploy，防止旧 run 在新版本之后发布。GitHub Pages environment 仍提供部署串行保护。

### 10.4 Pages 管理员前置条件

在包含部署 workflow 的提交合并到 `master` **之前**，仓库管理员必须设置：

```text
Settings → Pages → Build and deployment → Source → GitHub Actions
```

若无法在合并前设置，则首个 push 只视为构建验证；启用 Pages 后通过 `workflow_dispatch` 完成首次部署。不得把首次 deploy 失败误报为实现成功。

最终验收需保留以下至少一种证据：

- GitHub Pages 设置/API 显示站点已启用；
- `deploy-pages` job 成功并输出 `page_url`；
- 实际站点 URL 返回成功并能加载首页资源。

## 11. VitePress 配置

生产 `base` 为：

```text
/agentscope-rust/
```

配置启用：

- `cleanUrls: true`；
- 本地搜索；
- 明暗主题；
- 严格 dead link 策略，不使用全局 `ignoreDeadLinks: true`；
- 显式 nav/sidebar；
- GitHub 社交链接。

构建产物地址：

```text
https://ningning0111.github.io/agentscope-rust/
```

## 12. 错误处理

以下情况必须使 CI 失败：

- 仍有 `.mdx` 正式页面或正式 `.md` 页面数不是 50；
- mirror map、文件系统、sidebar 集合不一致；
- 未登记组件或关键组件属性；
- 残留 JSX `cols={数字}`；
- frontmatter、Markdown/Vue 模板或 SSR 构建错误；
- 残留 `/versions/0.1.0/zh/` 路由；
- 公开页面链接内容根外维护文件；
- 本仓库 GitHub 源码 URL 指向本地不存在路径；
- 站内链接、锚点或静态资源失效；
- `npm ci` 与 lockfile 不一致；
- artifact 目录不存在；
- dist 中出现排除内容或错误 base path。

不得通过关闭 dead-link 检查、忽略未知组件或扩大公开目录来绕过失败。

## 13. 内容正确性审计

公开文档中的示例 crate、`cargo run -p ...` 命令和源码路径需与当前仓库交叉验证：

- 无凭据即可验证的示例至少执行 `cargo check -p <package>` 或确认 package/路径存在；
- 需要 API key 的运行命令不在 CI 中实际调用外部模型，但对应 package 必须可编译；
- 未实现能力保持「计划中」状态，不伪造示例；
- 无法验证且会误导读者的命令应修订或删除。

检查范围限于 50 个公开页面引用的命令和路径。

## 14. 验证策略

### 14.1 原型验证

在全量迁移前，四页原型必须覆盖：

- 首页：Card/CardGroup、旧绝对路由；
- FAQ：Accordion/AccordionGroup 和键盘交互；
- change log：行内 Badge；
- 深层 overview：相对链接、CardGroup 和 `:cols`。

四页需通过开发服务器、SSR build 和生产 preview。

### 14.2 自动化验证

```bash
npm ci
npm run docs:check
npm run docs:build
npm run docs:check-built
```

并执行文档引用的可编译示例检查。

### 14.3 浏览器 smoke 与无障碍

使用 Playwright 对生产 preview 执行最小 smoke：

- 桌面和移动 viewport；
- 首页必备文本与链接；
- 深层路由直接访问和刷新；
- nav/sidebar 展开与跳转；
- 搜索可打开并返回已知页面；
- dark mode 切换；
- Accordion 可通过键盘展开，ARIA 状态正确；
- Card 可聚焦和激活；
- 「计划中」状态文本在 DOM 中可断言。

使用 `@axe-core/playwright` 检查首页、FAQ 和一个深层内容页，不接受 serious/critical 可访问性问题。

### 14.4 CI 与生产验证

- PR workflow 中 deploy job 为 skipped；
- `master` push 或手动触发 build 成功后 deploy；
- artifact 路径与 dist 一致；
- deploy job 是唯一拥有 Pages/OIDC 权限的 job；
- 线上首页、深层页面和静态资源在 `/agentscope-rust/` 下可访问。

### 14.5 独立审查

实现完成后由独立 reviewer 检查 workflow 权限、公开边界、组件语义、路由迁移、页面集合和测试充分性；verifier 重新执行全部检查后才能声明完成。

## 15. 验收标准

1. 原型四页先通过开发、SSR build 和 preview，再完成全量迁移。
2. `docs/rust/zh/` 中有且仅有 mirror map 指定的 50 个正式 `.md` 页面，正式 `.mdx` 为零。
3. mirror map、文件系统和 sidebar 三个集合完全一致。
4. 七类实际 Mintlify 组件均被兼容，未知组件/关键属性检查 fail closed。
5. JSX 属性和旧 `/versions/0.1.0/zh/` 路由残留为零。
6. 内容根外维护链接为零；示例和源码链接使用有效 GitHub URL。
7. 产品型首页通过必备内容 smoke test。
8. 桌面/移动导航、搜索、暗色模式、深层路由和 Accordion 键盘操作通过 Playwright。
9. 代表页面无 serious/critical axe 问题。
10. `docs/python/`、维护元数据和内部设计文件不出现在 dist。
11. `npm ci`、`docs:check`、`docs:build`、`docs:check-built` 全部通过。
12. PR 不持有部署权限且不部署；`master` push 和手动触发可部署。
13. Pages 已启用，成功 workflow 输出可访问的 `page_url`。
14. 根 `README.md` 包含中文文档站入口。
15. 50 页引用的示例 package、源码路径和无需凭据的编译检查通过。

## 16. 预期变更范围

```text
package.json
package-lock.json
.nvmrc
README.md
.github/workflows/docs.yml
scripts/docs/check-docs.mjs
scripts/docs/check-built-site.mjs
docs/rust/README.md
docs/rust/mirror-map.md
docs/rust/zh/**/*.mdx  # 删除（Git rename 来源）
docs/rust/zh/**/*.md   # 新路径、链接与 Vue 模板语法
docs/rust/zh/.vitepress/**
tests 或 Playwright 配置文件
```

变更不涉及 Rust 公共 API。由于有 50 个文件改名和链接迁移，实施计划必须按 §4.3 的阶段设置检查点，避免在技术原型失败时留下大规模中间态。
