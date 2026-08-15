# 发布到 GitHub Pages（维护者手册）

AgentScope Rust 中文文档站通过 `.github/workflows/docs.yml` 构建并发布到：

<https://ningning0111.github.io/agentscope-rust/>

> 本文档是仓库维护文件，不进入公开站点。

## 触发方式

| 事件 | 行为 |
|------|------|
| `pull_request`（指向 `master`，命中 docs 相关路径） | 只检查与构建，**不部署**；build job 无 Pages 写权限 |
| `push` 到 `master`（命中 docs 相关路径） | 检查、构建并部署 |
| `workflow_dispatch`（手动触发） | 检查、构建并部署 |

## 首次发布前置条件（必须先在 GitHub 仓库设置）

1. 打开仓库 **Settings → Pages → Build and deployment → Source**。
2. 选择 **GitHub Actions**（不是分支部署）。
3. 尽量在合并包含 `docs.yml` 的提交到 `master` **之前**完成此设置。

若设置已完成，`master` 上的首次 push 会自动完成首次部署。

## 首次部署流程

1. 确认 `master` 上已包含 `.github/workflows/docs.yml`（可手动 push 或触发）。
2. 若首次 push 早于 Pages 启用，进入 **Actions → Documentation**，点击 **Run workflow**（`workflow_dispatch`）手动触发。
3. 等待 `build` job 成功；随后 `deploy` job 运行并输出 `page_url`。
4. 用浏览器验证：
   - 首页 <https://ningning0111.github.io/agentscope-rust/>
   - 快速开始 `/quickstart`
   - FAQ `/others/faq`
   - 深层路由 `/building-blocks/agent/overview`（直接访问 + 刷新）
5. 确认页面资源均以 `/agentscope-rust/` 为前缀加载。

## 排障

- **`deploy` job 失败且提示 Pages 未启用/未配置**：到 Settings → Pages 选择 GitHub Actions 为源，然后重新运行 `Documentation` workflow。不要为此改动 workflow 权限，也不要改为把构建产物推到分支。（`build` job 只读且不依赖 Pages 元数据，即使 Pages 未启用也能完成构建；Pages 未启用时失败只会出现在 `deploy` job。）
- **PR 上 `build` 正常但 `deploy` skipped**：这是预期行为，PR 不部署。
- **`docs:check` 失败**：多为页面集合不一致（mirror map / 文件系统 / sidebar 三方必须相等）、残留组件或旧路由；按 `ERROR:` 输出修复，不要通过放宽检查绕过。
- **`docs:check-built` 失败**：确认是在 `docs:build` 之后运行；检查 dist 中是否有 `/agentscope-rust/` 前缀缺失或公开边界泄露。

## 本地全量验证（发布前建议执行）

```bash
npm ci
scripts/check-docs-mirror.sh --quiet
npm run test:docs
npm run docs:check
npm run docs:check-examples
npm run docs:build
npm run docs:check-built
npx playwright install chromium
npm run test:docs:e2e
```

全部通过后再 push 到 `master`。
