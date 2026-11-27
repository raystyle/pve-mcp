# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**pve-mcp — Proxmox VE 虚拟化平台 MCP server**。

对 PVE 集群做查询与受控管理的本地 MCP 服务。**架构参考 `D:/repos/know-mcp`**（Rust workspace：`pve-core` 引擎 + `pve-server` 入口；daemon 探测-拉起-等就绪模式、锁文件单实例、stdio + HTTP 双 MCP 传输、CLI 动词、TOML 配置）。**功能面参考 `raystyle/pve-harness` rust 分支**（私有仓，经 `gh api repos/raystyle/pve-harness` 访问）。认证双支持：API Token 优先 + 用户名密码 ticket 续签。

主开发机 Windows（`D:\repos\pve-mcp`）。连接目标是用户的 PVE 集群——**MCP 工具面破坏性操作（stop/destroy/rollback）必须 confirm 护栏**。

## 项目内 Claude 配置分工

职责表与"什么放哪"判据见 **`.claude/README.md`**（必读）：

- **CLAUDE.md**（本文件）：每会话必备事实——定位、红线、导航
- **`.claude/rules/`**：路径域规则（`rust-dev.md` 管 crates/构建脚本；`adr-docs.md` 管 adr/docs；`skill-authoring.md` 管 skills；`docs-style.md` 管受控双语）
- **`.claude/skills/`**：过程知识（`/precedent` 先例搜索；`/adr` ADR 登记；`/verify` 验证；`/self-reflection` 自省）
- **`.claude/memory/`**：项目记忆（Claude 自维护，经 `settings.local.json` 的 `autoMemoryDirectory` 指向仓内）

## 纪律红线（每个会话都要遵守）

- **PVE 凭据红线**：API token secret、用户密码只经环境变量或 gitignored 的 `pve-mcp.toml`；绝不进 git、日志、evidence、仓内文档。`pve-mcp.toml.example` 用占位符。连接信息（主机/用户/realm）写文档时脱敏。
- **破坏性操作护栏**：MCP 工具面一切破坏性 PVE 操作（stop/shutdown/destroy/rollback/migrate）必须 `confirm=true` 参数；CLI 动词面必须 `--confirm`。无护栏实现 = blocking。
- **ADR 只记用户决定**：每条 ADR 必须经用户确认后写入 `.ad_records/`；Claude 只起草，不擅自落笔。参考资料不记为需求。
- **新需求先搜先例**：先查本地 `D:/repos/know-mcp`（架构先例）与 `pve-harness` rust 分支（功能先例）→ gh-mcp 搜同类实现 → 再设计；流程见 `/precedent`。
- **不直打生产集群做开发测试**：对真实 PVE 的测试只做只读冒烟；写操作测试须用户明示批准或指向专用测试节点。
- **语言约定**：文档中文；代码标识符、注释、commit message 英文。
- **资产自包含**：CLAUDE.md / rules / skills / REVIEW.md 不写来源署名、ADR 编号、历史轨迹——面向无上下文的读者，直接可执行。

## 开发流程导航

- **主闭环**：Research → Plan（人工批准）→ Implement（TDD）→ Verify → 独立审查 → Ship；失败回路 ≤3 轮。产物链落 `.research/<feature>/`（进 git）。
- **审查**：规则正本根目录 `REVIEW.md`（三档分级，从 main 读取；无 main 时降级读工作区并向用户明示）；审查代理 `.claude/agents/reviewer.md`（代码+安全一体，新上下文、未参与编写）；审查结论归档 `.review/`
- **决策跟踪**：`.ad_records/`（需求/进度状态/最终产物/是否满足四节，索引见 `.ad_records/README.md`）
- Rust 开发规则（构建门、测试纪律、daemon 模式战史坑）→ **`.claude/rules/rust-dev.md`**（触 crates/构建时自动载入）

## 当前状态

- 开发体系闭环已落地（四资产 + hooks 检查门 + ADR + 审查体系 + system-lint 全局一致性核查）；检查门 `just ci` = fmt → clippy → nextest + doc tests + system-lint
- 代码进度：pve-core 域类型 + 配置 + API 客户端已交付并真实集群冒烟通过（PVE 9.2.21）；pve-server daemon 骨架（typestate 生命周期）进行中
- 连接配置 `pve-mcp.toml`（gitignored，模板 `pve-mcp.toml.example`）：单节点 lan-pve（192.168.88.88:8006）
- git main 已建立；远程 github.com/raystyle/pve-mcp 已配，未推送
