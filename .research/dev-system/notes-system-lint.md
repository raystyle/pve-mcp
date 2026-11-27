# system-lint — 全局 lint 体系先例研究与落地（2026-11-27）

> 用户指令："我们的项目没有建立完善的 lint，研究下先例，要建立全局 lint，从代码到文档到 CLAUDE code 配置和衍生工具"；追问查证"node js 可以给 rust 做 lint？"

## 先例查证

### Node 给 Rust 项目做 lint：有强先例，定位是"clippy 表达不了的层"

| 先例 | 证据 | 形态 |
| --- | --- | --- |
| **oxc-project/tsrs**（oxc 生态纯 Rust 编译器） | `tools/SCRIPTS.md` + `tools/lint/source.mjs`（551 行） | `vp run lint = node tools/lint/source.mjs && cargo clippy && node tools/check-scripts.mjs`——Node 脚本与 clippy 并列同门；source.mjs 自述定位 "Source checks which Clippy cannot express"（查 unsafe impl Send/Sync、原子 Ordering 需注释证明等） |
| **rust-lang/mdBook**（官方仓） | CONTRIBUTING.md | `npm run lint` 同时编排 cargo clippy 与 npm 工具链——官方项目自己组合 |
| **mikroverk/esp32sim** | CONTRIBUTING.md | `node tools/wasm-test.mjs` 与 cargo 门并列 |
| **vulnforge**（本机） | tools/system-lint.mjs（12 检查） | 资产图一致性（ADR 索引/研究库/hook-skill 形状/术语执法/测试↔工具对齐）——不碰产品代码 |

### 分层结论（判定）

| 层 | 权威工具 | 说明 |
| --- | --- | --- |
| Rust 代码语义 | **cargo clippy / rustc lints** | 语言层唯一权威，Node 不碰 |
| clippy 表达不了的文本模式 | Node 脚本（tsrs 模式） | 跨文件/仓库级约束 |
| 资产图一致性 | Node 脚本（vulnforge 模式） | 读 TOML/MD/JSON/git 状态，cargo 做不了 |

### vulnforge system-lint 的检查清单（研究源）

门文档对齐 / 工具文档覆盖 / ADR 索引双向 / 研究库五件套 / hook 监视目录对齐 / 状态机可达性（ratchet 地板）/ 规则回响 / skill 形状 / hook 形状 / CLAUDE.md 时效性 / 测试↔工具对齐 / 术语执法。全部"事实↔衍生文档"对齐模式：**manypkg check-registry 形态**（每个检查 = 命名单元：validate → findings → 聚合 exit）。

## 落地：tools/system-lint.mjs（10 检查）

适配 pve-mcp 资产图：

| 检查 | 事实源 | 衍生物 |
| --- | --- | --- |
| gate-docs-alignment | justfile ci recipe | rust-dev rules / verify skill / README |
| config-example-sync | crates/pve-core/src/config.rs 字段 | pve-mcp.toml.example |
| adr-index-alignment | .ad_records/NNN-*.md | README 索引行（双向） |
| research-feature-alignment | .research/<feature>/ 目录 | README feature 表（双向） |
| hook-shape | .claude/settings.json 引用 | hooks/*.mjs 磁盘存在 |
| skill-shape | .claude/skills/*/ | SKILL.md + name/description frontmatter |
| rule-frontmatter | 预期 rule 文件 | paths 域声明 |
| docs-terminology | docs-style.md 术语表 | 全中文 markdown 语料（门禁/闸门/确认门/虚机/危险操作 → 检查门/护栏/虚拟机/破坏性操作） |
| **credential-redline** | pve-mcp.toml 真实密码 | 全仓扫描泄漏 + git untracked + example 占位符 |
| claude-md-current | — | CLAUDE.md 必须提及 just ci 与 pve-mcp.toml |

## 首轮战绩

- 首跑抓 **21 处真实违例**：example 缺 8 字段、研究库表缺 3 行、术语违例 10 处（含 ADR/rules/skills 全层面）
- 全部修复后 10/10 通过；红→绿证明：破坏 ADR 索引行（001→999）→ lint 报 finding → 恢复 → 全绿
- 接入 justfile：`ci: fmt lint test test-doc system-lint`

## 后续候选（未做，按 Miss→Rule 晋级）

- 测试↔工具对齐（工具面落地后：每个 MCP 工具名有按名测试）
- 工具表注册表投影（ADR-001 工具落地后：代码注册表 ↔ docs 工具表互查，NANDI TOOLS.md CI 再生模式）
