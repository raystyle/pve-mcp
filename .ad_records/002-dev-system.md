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
| 3 Rust 域适配     | ✅ 完成  | —（justfile 已落地，45 测试在跑）|
| 4 hooks 检查门    | ✅ 完成  | —（.git/HEAD 门标记已接；CRLF 经 .gitattributes 修复） |
| 5 独立审查        | 🔄 进行中 | reviewer 代理首轮审查跑批中（规则从 main 读） |
| 6 决策跟踪        | ✅ 完成  | 001/002 均实施中                 |
| 7 受控双语        | ✅ 完成  | 术语执法进 system-lint 实战（抓 10+ 违例） |
| git init + 远程   | ✅ 完成  | main 已推送 github.com/raystyle/pve-mcp |
| 三平台 peer 轮    | ✅ 首战全绿 | 每次推送后常态轮（verify skill 固化） |
| 全局 lint         | ✅ 完成  | system-lint 10 检查进 just ci    |

## 最终产物

| 产物           | 路径                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------- |
| 四资产全套     | `CLAUDE.md`、`.claude/README.md`、`.claude/rules/`（4 文件）、`.claude/skills/`（5 skill 含 pve-api）、`.claude/memory/` |
| 审查体系       | `REVIEW.md`、`.claude/agents/reviewer.md`、`.review/`                                          |
| 决策跟踪       | `.ad_records/`（README + 001 + 002）                                                          |
| hooks 检查门   | `.claude/hooks/`（guard-dangerous / verify-evidence / archive-session / with-node）           |
| 配置           | `.claude/settings.json`（权限 + hooks + plansDirectory）、`.claude/settings.local.json`、`.mcp.json`（项目级，untracked） |
| 研究库         | `.research/`（README + dev-system/ + feature-scan/ + peer-round/）                             |
| 全局 lint      | `tools/system-lint.mjs`（10 检查）+ `.gitattributes`（LF 钉死）                                |
| 构建门         | `justfile`（fmt → clippy → nextest → doc → system-lint）                                       |
| 先例研究       | `.research/dev-system/`（体系/rust 模式/PVE 客户端/system-lint 四份 notes）                    |

## 是否满足需求

**满足**（2026-11-27 判定）：体系闭环全环运转——首战三平台 peer 轮抓出 3 个 Windows 盲区 bug 并修复收口（`.research/peer-round/result.md`）；system-lint 实战抓 20+ 违例；hooks 在真实开发中两次自我执法（拦 ad-hoc pvesh 写、拦凭据入测试）。剩余：独立审查首轮结论归档、implement/ship 专项 skill。
