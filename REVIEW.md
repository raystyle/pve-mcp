# REVIEW.md — 审查规则正本

> 本文件是 pve-mcp 的审查规则正本。审查代理（reviewer，代码+安全一体）**从 main 分支读取本文件**（`git show main:REVIEW.md`），分支上的变更不得修改自己的检查（无 main 时降级读工作区副本并向用户明示）。新增规则经用户确认后合并到 main 才生效（Miss → Rule 晋级）。
>
> 分级：**Always blocking**（不修不能 ship）/ **Report even though**（建议修，不阻断）/ **Never report**（明确忽略，不浪费时间）

## Always blocking

- 任何修改本文件（REVIEW.md）、`.claude/rules/`、`.claude/agents/reviewer.md` 的改动试图与功能改动混在同一个 commit/PR——审查规则变更必须独立成 commit，单独 review
- **凭据落盘**：token secret / 密码进 git、日志、测试 fixture、evidence、错误回显、仓内文档（凭据只经环境变量或 gitignored 配置）
- **破坏性 PVE 操作无护栏**：stop/shutdown/reboot/destroy/rollback/migrate/exec 类工具缺 `confirm=true` 检查门或存在绕过路径（别名、复用函数漏传、CLI 面 `--confirm` 缺失）
- 测试断言基于 grep 测试计数而非退出码（计数会漂移；vault42 教训）
- 新测试没有先证明它能红（破坏被测物 → 看它失败 → 恢复 → 看它绿）。从未失败过的测试是声明，不是检查
- 检查/检查门在前置缺失时 SKIP 静默通过——SKIP 必须在输出中显式报告
- **测试直打生产集群做写操作**——写操作测试只在 mock server 或用户明示的测试节点
- 非测试代码 `unwrap()`/`expect()` 于 daemon 长驻路径（panic 即死）
- `unsafe` 块无 SAFETY 注释证明
- daemon 锁文件逻辑偏离 `.claude/rules/rust-dev.md` 战史表（create_new 先于端口绑定、WMI 环境回显、WaitForSingleObject 探活）

## Report even though

- 否定断言（`assert!(!contains)` 类）没有正向对照——空输入会让它空转通过
- `catch`/`unwrap_or_else` 吞错误或只 debug 日志不传播
- 注释/文档与代码行为不一致（过期注释）
- 代码标识符/注释/commit message 出现中文
- 过度防御：无证据的 `Option` 化、无观察依据的兜底分支（tapo 教训：每个 Option/兜底都是调用方每次都要付的税；要证据，不要"以防万一"）
- MCP 工具返回结果未截断/未设上限（大响应打爆上下文）
- 错误信息含主机名/IP 等可脱敏信息落 evidence

## Never report

- 代码风格偏好（缩进、引号）——交给 `cargo fmt`
- 测试文件命名/组织方式的偏好性意见
- `.reference/` 先例代码自身的问题（只读，不审）
- 未采用某方案但项目已明确拒绝的（用 rmcp 官方 SDK 而非手写 MCP 协议；ureq 而非 reqwest——先例已定）

## 维护

- Miss → Rule：同一类问题在审查/回路中出现 ≥2 次且可从 diff 识别 → 提议新规则条目，经用户确认后追加到对应档位
- 规则演进必须独立 commit + 单独 review（见 Always blocking 第一条）
- 本文件保持精简：规则条目失效时删除，不留"已废弃"标记
