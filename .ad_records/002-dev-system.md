# ADR-002 | Claude Code 开发体系

- 日期：2026-11-27
- 状态：实施中

## 需求

用户指令："学习 D:/repos/vulnforge 的形态，先为我们的项目构建起 claude code 开发体系结构闭环"。补充指令（会话中）：gh-mcp 研究 Rust 类项目怎么搭建 Claude Code 开发体系；不要忘记 rules。

子需求：

1. **四资产分工**（vulnforge 形态）：CLAUDE.md（每会话事实）/.claude/rules/（路径域规则）/.claude/skills/（过程知识）/.claude/memory/（仓内化记忆，经 settings.local.json）
2. **主闭环**：Research → Plan（人工批准）→ Implement（TDD）→ Verify → 独立审查 → Ship；失败回路 ≤3 轮；产物落 `.research/<feature>/`
3. **Rust 域适配**（gh-mcp 先例研究收口）：门表格化（fmt → clippy -D warnings → nextest + doc tests，`just ci` 承载）；变更三级分类（Feature/Bugfix/Trivial，不得中途降级）；nextest 而非裸 cargo test；review 必须留 record
4. **强制检查门 hooks**：guard-dangerous（本地危险命令 + PVE 变更动词拦截）/ verify-evidence（Stop 观察哨）/ archive-session（会话归档）；先例证据：Claude Code issue #34132 确认 rules 是 advisory-only，要紧规则必须转 hook
5. **独立审查**：reviewer 代理（代码+安全一体，新上下文）；REVIEW.md 三档分级正本（从 main 读取，降级工作区）；结论归档 `.review/`；PVE 集群证据分级（实连只读 > 官方文档 > 先例代码 > 推理）
6. **决策跟踪**：`.ad_records/` 四节格式 + 用户确认后写入纪律
7. **受控双语规范**：术语表 + 中文短句 + 代码命名三态对齐

## 进度状态

| 子需求            | 状态     | 剩余                             |
| ----------------- | -------- | -------------------------------- |
| 1 四资产          | ✅ 完成  | —                                |
| 2 主闭环约定      | ✅ 完成  | implement/ship 专项 skill 未建   |
| 3 Rust 域适配     | ✅ 完成  | justfile 随代码动工落地          |
| 4 hooks 检查门      | ✅ 完成  | git init 后接 .git/HEAD 门标记   |
| 5 独立审查        | ✅ 完成  | git init 后正本切 main 读取      |
| 6 决策跟踪        | ✅ 完成  | —                                |
| 7 受控双语        | ✅ 完成  | 术语表经使用增删                 |
| git init          | ❌ 未做  | 待用户决定；init 后首 commit 建 main |

## 最终产物

| 产物           | 路径                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------- |
| 四资产全套     | `CLAUDE.md`、`.claude/README.md`、`.claude/rules/`（4 文件）、`.claude/skills/`（4 skill）、`.claude/memory/` |
| 审查体系       | `REVIEW.md`、`.claude/agents/reviewer.md`、`.review/`                                          |
| 决策跟踪       | `.ad_records/`（README + 001 + 002）                                                          |
| hooks 检查门     | `.claude/hooks/`（guard-dangerous / verify-evidence / archive-session / with-node）           |
| 配置           | `.claude/settings.json`（权限 + hooks + plansDirectory）、`.claude/settings.local.json`       |
| 研究库         | `.research/`（README + dev-system/ + feature-scan/）                                           |
| 先例研究       | `.research/dev-system/notes.md`（tapo / keeler / #34132 结论）                                 |

## 是否满足需求

（验证后回填）
