---

paths:

- ".claude/skills/**"

---

# Skill 编写规范（agentskills.io 标准）

## 目录结构

```text
skill-name/          # 目录名 = frontmatter name，小写连字符
├── SKILL.md         # 必需：frontmatter + 指令
├── scripts/         # 可选：可执行脚本
├── references/      # 可选：详细文档（按需加载）
├── assets/          # 可选：模板、数据文件
└── evals/           # 可选：测试用例
```

## Frontmatter

```yaml
---
name: skill-name # 1-64 字符，小写+连字符
description: 一句话做什么 + Use when 触发场景 # 1-1024 字符，决定触发
compatibility: Requires uv and bun # 环境要求，有脚本必写
license: MIT # 可选
metadata:
  author: ray
  version: "1.0.0"
---
```

## 脚本规范（自包含，优先级：uv > Bun > npx）

- **Python**：`uv run scripts/x.py`，PEP 723 头声明依赖并 pin 版本（`# /// script ... ///`）
- **JS**：`bun run scripts/x.mjs`，版本化 import（`import x from "pkg@1.0.0"`）
- 禁交互式提示；提供 `--help`

## SKILL.md 本体

- < 500 行；细节下沉 `references/`（渐进式披露）
- 步骤化、可执行、面向无上下文读者；不写历史轨迹
- 纪律类条目（红线、护栏）用加粗或独立小节，确保不被长文本淹没
