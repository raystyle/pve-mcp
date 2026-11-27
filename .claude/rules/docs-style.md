# 受控双语规范（文档 + 代码命名）

> 参照 ASD-STE100（受控英语）、Microsoft Writing Style Guide（"one term, one concept"）、ruanyf/document-style-guide（中文技术文档写作）、W3C 中文排版需求。目标：降低跨 Agent 与人工审查的理解损耗；规范只约束**表达形式、术语、命名**，不替代事实核查与逻辑校验（归 /verify 与 REVIEW 独立审查）。

## 核心原则（四条，中英通用）

1. **一概念一术语**：同一概念在文档、代码、命令中各只有一个名字；禁近义词轮换
2. **短句**：中文单句 ≤60 汉字；英文指令句 ≤20 词
3. **一句一动作**：列表项不链式多动作（禁"然后/接着/再"串联，拆条）
4. **可追溯**：结论必须带证据（命令/退出码/测试数/evidence 文件）；主动语态

## 术语表（概念 ↔ 中文 ↔ 英文 ↔ 代码标识符）

| 概念           | 中文正术语 | 英文正术语       | 代码标识符                            | 中文禁用     |
| -------------- | ---------- | ---------------- | ------------------------------------- | ------------ |
| 管理面         | 管理面     | admin plane      | `handle_admin` / `admin_request`      | 管理接口     |
| 检查门         | 检查门     | check gate       | `just ci`                             | 门禁、闸门   |
| 护栏           | 护栏       | guardrail        | `confirm` 参数 / `--confirm`          | 确认门、拦截 |
| 破坏性操作     | 破坏性操作 | destructive op   | `stop`/`destroy`/`rollback` 工具      | 危险操作     |
| 单实例互斥     | 单实例互斥 | single-instance  | `daemon.lock`                         | 单例锁       |
| 端口文件       | 端口文件   | port file        | `admin.port`                          | 端口记录     |
| 冷启动         | 冷启动     | cold start       | `ensure_daemon`                       | 首次启动     |
| stdio 桥       | stdio 桥   | stdio bridge     | `run_mcp_stdio`                       | 桥接进程     |
| 凭据           | 凭据       | credential       | `token secret` / `ticket`             | 密钥（泛称） |
| 虚拟机         | 虚拟机     | VM               | `vmid`                                | 虚机（口语） |
| 节点           | 节点       | node             | `node`                                | 主机（泛称） |
| 集群           | 集群       | cluster          | `cluster`                             | —            |
| 快照           | 快照       | snapshot         | `snapshot`                            | 存档         |
| 先红后绿       | 先红后绿   | red before green | 测试纪律                              | —            |
| 证据           | 证据       | evidence         | `.research/<feature>/evidence/`       | 凭证         |
| 独立审查       | 独立审查   | independent review | `.claude/agents/reviewer.md`        | 复审         |

- **以"主机"指代 node 是最易犯违例**：node 指 PVE 集群成员；"宿主机"仅指 PVE 物理机语境（如 SMART 盘健康）
- 新术语进表：先查 know-mcp / pve-harness 既有用法，经用户确认后增删；发现违例改文不改表

## 中文规则

- 单句 ≤60 汉字；表格、代码、路径、命令、frontmatter 行豁免
- 中英文之间加空格（`PVE 集群`）；数字与单位同理
- 标点全角（中文语境）；括号内容为纯英文/代码时可用半角
- 主动语态："工具执行 X"，不用"X 被工具执行"

## 英文规则（STE 适配）

- 指令句 ≤20 词；禁口语缩写（don't → do not）
- 同一动作只用一个动词：load（不混用 fetch/fetch-in）
- 错误信息句式：`<what> failed: <reason>`；错误里不出现 "!" 感叹

## 代码命名（三态对齐）

- 术语表"代码标识符"列是唯一命名正源；文档、CLI 参数、MCP 工具名、Rust 标识符三态一致
- MCP 工具命名：`<域>_<动作>`（如 `vm_status` / `vm_start` / `snapshot_create`）；CLI 动词与 MCP 工具 1:1 映射（先例 know-mcp know-cli 形态）
