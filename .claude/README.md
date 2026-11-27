# .claude/ — 项目内 Claude 配置分工总纲

> 依据 Claude Code 官方机制（memory / skills / hooks / agents）与 vulnforge 先例（`D:/repos/vulnforge`），核心资产各司其职。**先例证据**：Claude Code issue #34132 官方确认 rules/CLAUDE.md 是 advisory-only（无强制机制），hooks 才是 enforced——"任何真正要紧的规则，从 rule 转成 hook"。

| 资产                       | 职责                                                                 | 写入者         | 载入时机            | 本仓内容                                                                    |
| -------------------------- | -------------------------------------------------------------------- | -------------- | ------------------- | --------------------------------------------------------------------------- |
| **`../CLAUDE.md`**（仓根） | 项目常驻上下文：定位 + 红线 + 资产导航，只放"每个会话都要知道的事实" | 人（用户定稿） | 每会话首载          | 项目定位 / 开发流程导航 / 纪律红线                                          |
| **`rules/*.md`**           | 路径域规则：只在对应对径的文件被触及时才载入（省上下文）             | 人             | 匹配 `paths` 时     | `rust-dev.md`（`crates/**` 等：Rust 开发规则）/ `adr-docs.md` / `skill-authoring.md` / `docs-style.md` |
| **`skills/<名>/`**         | 多步骤**过程**知识：按需载入（用户 `/` 调用或语义匹配），不常驻      | 人             | 调用时              | `precedent/` / `adr/` / `verify/` / `self-reflection/`                      |
| **`memory/`**              | 会话性状态 + 学习指针（可含历史轨迹）                                | Claude（auto） | 每会话（首 200 行） | MEMORY.md 索引 + 各记忆文件                                                 |
| **`sessions/`**            | 会话归档：SessionEnd hook 镜像 transcript 到 `.claude/sessions/<id>.jsonl`（实体不进 git），INDEX.md 索引进 git | hook（auto）   | 会话结束            | 会话记录（可含轨迹）                                                        |
| **`plans/`**               | Plan Mode 落盘区（plansDirectory）；批准后归档 `.research/<feature>/plan-snapshot.md` | Claude         | 生成时              | 历史计划（可含轨迹）                                                        |

辅助资产：

| 资产                | 职责                                                            | 写入者 | 本仓内容                                              |
| ------------------- | ---------------------------------------------------------------- | ------ | ----------------------------------------------------- |
| **`settings.json`** | 权限 allowlist + hooks 配置 + plansDirectory                    | 人     | cargo/just/git 常规操作 allowlist + 三 hooks          |
| **`hooks/**`**      | **确定性强制**：必须拦的动作，CLAUDE.md 只是约定、hook 才是检查门 | 人     | guard-dangerous.mjs / verify-evidence.mjs / archive-session.mjs + with-node.sh |
| **`agents/reviewer.md`** | 独立审查代理定义（新上下文、代码+安全一体）                 | 人     | 审查流程 + PVE 证据分级                               |

## 会话反哺闭环

会话数据是体系进化的原料：

1. **归档**：SessionEnd hook 镜像 transcript 到 `.claude/sessions/`（`PVE_MCP_SESSION_ARCHIVE=0` 可关）
2. **分析**：用 `/self-reflection` 读会话 transcript，提炼修正模式（用户纠正/反复返工/纪律违规）
3. **提炼**：从分析中识别重复模式
4. **晋级**：按 Miss→Rule 流程沉淀——出现 ≥2 次的模式进 rules/skill/hook；单次重大决定进 ADR；先例与证据进 `.research/`

## 判据（什么放哪）

- 每个会话都要的**事实** → CLAUDE.md；只对某类文件要的**规则** → rules（paths 域）
- **过程性**知识（步骤/流程）→ skill；**跨会话工作状态** → memory；**强制不可违反** → hook
- 决策/需求固化 → `.ad_records/`，memory 只留指针
- **历史轨迹边界**：ADR、PR（commit）、memory、plan 允许轨迹；rules/skills/CLAUDE.md/README/研究文档自包含无轨迹

## Rust 项目特化（先例依据）

- **检查门用 just 承载**（`just ci`）：Rust 生态标准门 = `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo nextest run --workspace` + doc tests；先例 tapo/keeler/flockr 同款门表格化
- **测试跑 nextest 而非裸 cargo test**：env/cwd 敏感的测试在 cargo test 线程模式下假红（先例 flockr AGENTS.md 明示，know-mcp 同款实践）
- **审查必须留 record**（`.review/`）：keeler 教训——review 阶段不留 artifact 时被连续跳过 20 个 task 而无人察觉
