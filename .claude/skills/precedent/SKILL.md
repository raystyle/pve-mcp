---
name: precedent
description: 先例搜索流程：新需求/写实现前，先查本仓先例（D:/repos/know-mcp 架构、pve-harness rust 分支功能面），再用 gh-mcp 搜同类实现并研究先例的错误与 bug，深入分析可 clone 到 .reference/。当接到新需求、设计新模块、或需要确认某功能的业界做法时使用。
---

# precedent — 先例搜索流程

## 步骤

1. **先查本地产物**：优先检索 `.research/`（既有研究结论，防重复研究）与本地先例仓：
   - 架构先例：`D:/repos/know-mcp`（daemon 模式、rmcp、workspace 形态、战史坑见 `.claude/rules/rust-dev.md`）
   - 功能先例：`raystyle/pve-harness` rust 分支（私有仓，`gh api repos/raystyle/pve-harness/...?ref=rust` 访问；功能清单在 `harness/src/commands/mod.rs` 的 REGISTRY）
2. **明确要搜什么**：把需求拆成可搜索关键词（技术名 + 场景），如 `rmcp tool_handler stdio bridge`、`proxmox api2 json rust client`
3. **gh-mcp 搜索**（本地先例无果后）：
   - `search_repositories`：找同类项目（star 数与活跃度判成熟度）
   - `search_code`：找具体实现片段
   - `get_file_contents`：读关键文件（README、CLAUDE.md/AGENTS.md、Cargo.toml、测试、CI 配置）
4. **web 搜索**（gh-mcp 无果的最后手段）：官方文档（PVE API viewer、rmcp docs）、标准、博客
5. **重点研究三件事**：
   - 先例**如何完成**需求（架构、目录、API 用法）
   - 先例**犯过什么错**（issue、CHANGELOG、AGENTS.md 里的战史规则）
   - 先例**出过什么 bug**（教训如何固化为 lint/测试/规则）
6. **需要深入分析时**：`git clone --depth 1 <repo>` 到 `.reference/<repo-name>/`（不进 git），本地工具分析
7. **产出**：研究结论汇总给用户；结论落 `.research/<feature>/notes.md`；教训提炼为 ADR 候选条款或 rules 候选规则，经用户确认后固化

## 纪律

- `.reference/` 只读不进 git（.gitignore 排除）；clone 用 `--depth 1`
- 研究结论拿不准是否算用户需求时，先问用户再记录（ADR 只记用户决定）
- Rust 项目的 Claude 开发体系先例参考（已归档结论）：tapo（精简 CLAUDE.md + 过程技能 + review-loop 证据分级）、keeler/cargo-crap（门表格化、变更三级分类、review 必须留 record）、flockr（nextest 而非裸 cargo test 的原因）
