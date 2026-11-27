---
name: verify
description: 验证流程：跑检查门（just ci：fmt → clippy -D warnings → nextest）+ 采集防假测试证据（退出码断言、测试数非零、新测试先红后绿、verbose 输出落盘 .research/<feature>/evidence/）+ PVE 实连只读冒烟 + 三平台 peer 轮（lan-mac/lan-ubuntu 常驻 Claude 会话经 herdr 联动）。当实现完成待验证、ship 前、或用户要求"验证这个改动"时使用。
---

# verify — 验证与证据采集流程

## 步骤

1. **确定 feature 名**：当前工作对应 `.research/<feature>/`；没有则先建（目录约定见 `.research/README.md`）。
2. **跑检查门**：

   ```bash
   just ci   # cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && cargo test --doc --workspace && node tools/system-lint.mjs
   ```

   断言**退出码为 0**。部分命令缺失（nextest/just 未装）时显式报告"哪项被跳过"，不得静默当作通过。无代码阶段（脚手架期）显式声明门不适用并跳到第 5 步。
3. **防假测试证据**：
   - 测试数非零断言：nextest 输出必须含通过数且 N > 0；N=0 的"全绿"是失败
   - **新测试先红后绿**：本次新增的测试，先故意破坏被测逻辑确认它失败，再恢复确认通过；过程记入 evidence
   - 如有 SKIP 的测试，逐条列出原因
4. **证据落盘**：

   ```bash
   cargo nextest run --workspace 2>&1 | tee .research/<feature>/evidence/nextest.log
   ```

   失败与通过均保留；evidence 只留关键文件，超大日志先裁剪。**evidence 与日志中不得出现凭据**（token/密码/主机名脱敏后再落盘）。
5. **PVE 实连只读冒烟**（涉及 API 客户端改动时）：
   - 凭据经环境变量：`PVE_TEST_HOST` / `PVE_TEST_TOKEN_ID` / `PVE_TEST_TOKEN_SECRET`（或 `PVE_TEST_USER`/`PVE_TEST_PASSWORD`）
   - 缺失时测试 SKIP 并显式报告；**只打 GET 请求**（/version、/nodes、/nodes/{node}/qemu），任何写操作不在冒烟面内
   - 冒烟结果（节点数、VM 数、延迟）记入 evidence
6. **三平台 peer 轮**（本地门绿后）——见下节。
7. **写 result.md**（若 feature 流程中）：跑过的命令、退出码、测试数、SKIP 清单、先红后绿记录、冒烟结果、三平台逐台结果。
8. **失败回路**：失败 ≤3 轮——修实现（回 Implement）/改计划（回 Plan）/重新研究（回 Research）由你判断并在 result.md 记录选择了哪条；第 3 轮仍失败强制停下，报告用户人工介入。

## 三平台 peer 轮（跨机器联动验证）

主开发机（Windows）本地门全绿、推送 main 后，lan-mac / lan-ubuntu 常驻 peer 会话（`~/repos/pve-mcp`，经 herdr `hc` 驱动）拉取并跑同样的门。Windows-only 代码（WMI 分支）在 unix 上不编译——**unix 分支的真实性只有 peer 轮能暴露**（首战战例：E0382 move 错误 + unix 子进程 arg 过期，双 bug 同轮抓出）。

**peer 会话生命周期**：会话常驻（用户裁定 2026-11-27），一轮一任务；名字 `pve-peer-mac`（lan-mac/t1p）、`peer-ub`（lan-ubuntu/t2c）。**herdr 地址是会话级资源**——重启/断线后用 `hc ls` 重查，短名失效就打全地址 `machine/tXX`。

**操作序列**：

1. `git push origin main`（peers 从远端拉，不直接推 peers）
2. 派发（全地址）：

   ```sh
   hc send lan-mac/t1p "cd /Users/ray/repos/pve-mcp && git pull --ff-only; just ci; 汇报门退出码+测试数"
   hc send lan-ubuntu/t2c "cd /home/ray/repos/pve-mcp && git pull --ff-only; just ci; 汇报门退出码+测试数"
   ```

3. 长跑用 Monitor 等完成信号（`until hc read <addr> | grep -q ...`），不轮询
4. 逐台收报：门各阶段退出码 / 测试数 / SKIP 清单；失败要求贴失败测试名与原因
5. 三台（含本机）全绿才收口进 result.md

**坑表（pve-mcp 版，逐台排查先查这些）**：

| 症状 | 根因 | 处置 |
| --- | --- | --- |
| `spawn_failed: cannot start "/bin/bash"` | `--cwd ~/...` 的 `~` 不展开 | `--cwd` 用绝对路径（`/Users/ray/...`、`/home/ray/...`） |
| peer 名 `not_found` | 短名未在本机注册 | `hc send` 用全地址 `lan-mac/t1p` |
| `name_taken` | 旧 vulnforge peer（peer-mac/peer-linux）占名 | 换新名（pve-peer-mac 等）；勿 kill 用户旧会话 |
| 信任/imports 对话框挡住 | 新 clone 仓首次进入 | `hc keys <addr> down enter` 两步：信任目录 → 允许外部导入 |
| `NEXEST=missing` | peer 未装 cargo-nextest | 派发任务里带 `cargo install cargo-nextest --locked` 前置步 |
| Monitor idle 误报 |  state=done 时 terminal 可能仍在跑（build 中） | 等待条件用  而非 done |
| install 时 peer 机无 toml | pve-mcp.toml 是 gitignored 凭据文件，peer 机没有 | 冷启动 fail-closed 是正确行为；实调测试需先给 peer 配置（或 SKIP 声明） |
| update 遇 ETXTBSY (os error 26) | Linux 内核锁活进程持有的二进制 | install 用 temp+rename 原子替换（已修） |
| 假绿（GATE_EXIT=0 但报错） | `just ci ... ; echo` 分号吞退出码；just 配方内管道 `cmd | tail; echo EXIT=$?` 拿的是 tail 的退出码 | 派发语句用 `&&` 链；让 peer 报每阶段退出码而非只报最后 echo |

**peer 轮失败分类（三类处置不同，混同会误修）**：
- **平台真实差异**（unix 分支编译错/行为差）→ 本机修代码，`cfg(unix)` 分支同样要过 clippy——考虑本地 `cargo check --target` 交叉检查可行性
- **环境缺失**（工具未装/路径不同）→ peer 侧装，或任务里前置安装步
- **陈旧状态**（旧 clone/半写状态）→ peer 侧 `git fetch && git reset --hard origin/main` 后重跑

## 变更分级（选择最轻且诚实的级，不得中途降级躲门）

| 级别      | 适用                             | 门                                             |
| --------- | -------------------------------- | ---------------------------------------------- |
| Feature   | 新行为、改行为、新 API/工具      | 全流程：研究 → 计划 → TDD → ci → 审查          |
| Bugfix    | 既有行为错误                     | 先写失败回归测试 → 最小修复 → `just ci`        |
| Trivial   | 文档、注释、配置、重命名         | `cargo fmt --check`（动了代码则加 clippy+test）|

判据：如果拿不准是否改变行为，就不是 Trivial；如果"Trivial"改动让任何测试失败，重新分类（先例 keeler）。
