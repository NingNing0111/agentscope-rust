# `tool` 示例

自定义工具示例：FunctionTool 自动生成 schema，ToolKit 注册，并演示内置工具（Bash/Read/Write/Edit/Grep/Glob/ResetTools/Skill）。

## 运行

```bash
cargo run -p tool -- --help
```

## 凭据

真实模型调用需要环境变量：

```bash
export DASHSCOPE_API_KEY="sk-your-key"
```

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

<!-- 实现时补充：运行命令 + 预期输出 -->
- 有凭据时：……
- 无凭据时：输出明确的缺凭据错误
