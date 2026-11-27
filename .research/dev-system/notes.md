# dev-system — Rust 项目 Claude Code 开发体系先例研究

> gh-mcp 先例搜索（2026-11-27），服务 ADR-002。结论已固化进 `.claude/rules/rust-dev.md`、`.claude/README.md`、`.claude/skills/verify/SKILL.md`；本笔记留证据链。

## 检索路径

- `search_repositories`: "claude code rust CLAUDE.md rules skills"（2 命中，均工具类非范式仓）
- `search_code`: `"clippy -D warnings" "just ci" "cargo nextest"`（46 命中，展示前 15）
- `search_code`: `"path:.claude/rules" cargo clippy`（0 命中——`path:` 限定对 rules 目录检索无效，换个角度：直接看已知成熟仓的 .claude 结构）
- `get_file_contents`: tapo 的 `.claude/` 树 + CLAUDE.md + 两个 SKILL.md；cargo-crap 的 `.claude/keeler.md` 全文
- 转引命中：ManasHardas/workledger `plans/research/gap-scout.md` 调研了 Claude Code issue #34132

## 核心发现

### 1. Rust 生态标准检查门（多先例一致，表格化呈现）

tapo CLAUDE.md / keeler / reticle / itala / dataseek / canopus / systemrs 全同款：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace --all-targets (+ cargo test --doc)
just ci    # = lint + test 聚合
```

- **just 承载门**是 Rust 生态共识（12/15 命中用 justfile）；CI = `just ci` 逐字复用（sojourn：`CI is exactly just ci`）
- keeler 加 coverage（cargo-llvm-cov，changed code 零未覆盖线）、CRAP 基线棘轮、mutants 变异测试——重门，本项目暂不采纳（无此规模），记为可选升级

### 2. nextest 而非裸 cargo test（flockr AGENTS.md 明示原因）

> Plain `cargo test` runs tests as threads in one process and some tests that touch env vars or the working directory fail there; that is not a regression.

进程隔离。know-mcp `.config/nextest.toml`（slow-timeout/terminate-after）同款实践。本项目 daemon 互斥/端口文件测试强 env 敏感——必须 nextest。

### 3. CLAUDE.md 形态（tapo 29 行 vs keeler 194 行）

- tapo：极简——workspace 成员 + 跨语言绑定规则 + "首次修复失败先退一步找根因" + commit 约定指向 /commit skill。过程知识全部外置 skills
- keeler：完整流水线内嵌（spec→tasks→tdd→qa→review→mutants）+ 变更三级分类 + graph mode 并行多 agent
- 取舍：vulnforge 形态（CLAUDE.md 只放事实+红线+导航，过程进 skills）与 tapo 一致，维持；keeler 的变更三级分类收进 verify skill

### 4. advisory vs enforced（workledger 转引 Claude Code issue #34132，官方 closed not_planned）

> yurukusa 评论：CLAUDE.md rules: advisory … Hooks (exit code 2): **enforced** — the tool physically cannot execute. For any rule that matters, convert it from a rule to a hook.

官方确认 rules/CLAUDE.md 无强制机制。**判据固化**：要紧规则（危险命令、凭据）必须 hook；rules 只省上下文不作闸门。

### 5. review 必须留 record（keeler 原文教训）

> **The review stage leaves no artifact.** … In the repository that ships Keeler it was skipped for twenty tasks before anyone did.

其余门失败会响，review 不响——需要 `.review/` 归档 + 索引 + reviewer 代理纪律条目（已落地）。

### 6. tapo review-loop 的证据分级（与 PVE 场景同构）

tapo 是 IoT 设备库：审查发现中"设备会/不会发这个字段"类断言，用**真机只读调用**证实，不采信反编译 app 与 reviewer 推理；无法证实的标 unconfirmed 并说明哪次调用可定夺。

pve-mcp 直接同构迁移（reviewer.md 的"PVE 集群证据分级"节）：真实集群只读 GET > 官方文档 > 先例实现 > reviewer 推理。

### 7. tapo 的 MCP 工具面形态（本项目直接对标）

tapo 是"库 + Python 绑定 + MCP server"三面一体；pve-mcp 是"core + server(daemon/MCP/CLI)"一体——know-mcp 形态已定，不采 tapo 的 PyO3 面。

### 8. 查询方法论修正（本次实操教训）

- `search_code` 的 `path:.claude/rules` 带引号整串传会 0 命中；多限定词组合命中率低
- 高效路径：`search_repositories` 定位成熟仓 → `get_file_contents` 直接读 `.claude/` 树与关键文件（绕过弱检索）
- 私有仓（pve-harness）走 `gh api`（本机 CLI 有权限）

## 未采纳（明确拒绝，勿重研）

- keeler 的 graph mode（tmux 多 agent 并行）——规模不需要
- keeler 的 mutants/CRAP 基线——门过重；项目长大后可再评估
- claude-mem 等第三方记忆方案——用仓内 memory + sessions 归档已闭环
