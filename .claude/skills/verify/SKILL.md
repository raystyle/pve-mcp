---
name: verify
description: 验证流程：跑检查门（just ci：fmt → clippy -D warnings → nextest）+ 采集防假测试证据（退出码断言、测试数非零、新测试先红后绿、verbose 输出落盘 .research/<feature>/evidence/）+ PVE 实连只读冒烟。当实现完成待验证、ship 前、或用户要求"验证这个改动"时使用。
---

# verify — 验证与证据采集流程

## 步骤

1. **确定 feature 名**：当前工作对应 `.research/<feature>/`；没有则先建（目录约定见 `.research/README.md`）。
2. **跑检查门**：

   ```bash
   just ci   # cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && cargo test --doc --workspace
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
6. **写 result.md**（若 feature 流程中）：跑过的命令、退出码、测试数、SKIP 清单、先红后绿记录、冒烟结果。
7. **失败回路**：失败 ≤3 轮——修实现（回 Implement）/改计划（回 Plan）/重新研究（回 Research）由你判断并在 result.md 记录选择了哪条；第 3 轮仍失败强制停下，报告用户人工介入。

## 变更分级（选择最轻且诚实的级，不得中途降级躲门）

| 级别      | 适用                             | 门                                             |
| --------- | -------------------------------- | ---------------------------------------------- |
| Feature   | 新行为、改行为、新 API/工具      | 全流程：研究 → 计划 → TDD → ci → 审查          |
| Bugfix    | 既有行为错误                     | 先写失败回归测试 → 最小修复 → `just ci`        |
| Trivial   | 文档、注释、配置、重命名         | `cargo fmt --check`（动了代码则加 clippy+test）|

判据：如果拿不准是否改变行为，就不是 Trivial；如果"Trivial"改动让任何测试失败，重新分类（先例 keeler）。
