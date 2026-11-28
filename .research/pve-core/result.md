# pve-core 首增量结论（2026-11-27）

> feature: pve-core 域类型 + 配置 + API 客户端（ADR-001 子需求 1/3 起步）。

## 跑了什么

| 门 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过（0 警告） |
| `cargo nextest run --workspace` | 36 通过 / 0 失败 / 0 SKIP |
| `cargo test --doc --workspace` | 通过（0 doc tests） |
| 冒烟（`--features smoke`，真实集群只读） | 2 通过：login（密码通道）+ /version（PVE 9.2.21）+ /nodes（lan-pve online） |

## 先红后绿

- `vmid_rejects_out_of_range`：破坏 Vmid 范围校验（MAX+1）→ 红 → 恢复 → 绿 ✅
- `client_rejects_header_hostile_token`：绕过 header 校验（`if false &&`）→ 红 → 恢复 → 绿 ✅

## SKIP 清单

- 无（冒烟测试凭据已供给；无凭据环境会显式 SKIP 并报告）

## 交付物

- `crates/pve-core/src/domain.rs`：品牌域类型（Vmid/VmidRange/VmidSet/NodeName/StorageName/SnapshotName/Upid/GuestType/GuestRef）
- `crates/pve-core/src/config.rs`：pve-mcp.toml 解析 + validate（protected_vmids 加载期解析）
- `crates/pve-core/src/client.rs`：PveClient（token/ticket 双认证 + rustls 自签跳过验证 + 错误分层 + 信封解包 + ticket FSM）
- `crates/pve-core/tests/smoke_test.rs`：真实集群只读冒烟（smoke feature 门控）

## 实测发现

- ureq 2.12 与 3.x API 大改（AgentBuilder/Error::Status/set vs header）；先例 pve-harness 锁 ureq 2 正确
- rustls 0.23 的 danger 路径需要 `dangerous_configuration` 已改名（features: ring/std/tls12），NoVerify 需要 Debug 派生
- 错误 body 的 errors map 值经 serde Value Display 会带引号——`as_str` 优先

## 回路

无失败回路（首次全绿）。

## 补充轮（2026-11-27，token 主通道 + 先例对照 review）

- pvebot-ro@pve!mcp token 接入（ADR-001 裁定 #7）：本地配置切 token 主通道，密码降为注释备用；read 面全绿（cluster/list/status 106）
- 先例对照（agent-cloud / pve-events / flutter_server_box）：403 消息必须点名角色与修复路径——client.rs 401/403 消息改为 actionable（Forbidden 报文含 PVEAuditor + Propagate 提示）；新测试 forbidden_message_names_the_fix 先红后绿
- flutter_server_box 补充常量：TICKET_LIFETIME 2h、RENEW_AFTER 1h、LIFETIME_MARGIN 5min（时钟漂移+在途请求）——我们 10min margin 已覆盖同语义

## 覆盖度轮（2026-11-28，50→67 测试）

- 盘点发现 dispatch 层（16 个 admin arms）零直接测试；两级补齐：
  1. 纯函数套件（tool_surface_test.rs，9 项）：PowerAction confirm 分层/幂等 target/FromStr、SnapshotName 注入墙、Upid hex-pid 回归+node 提取、VmidSet 两种 live 形态
  2. mock-PVE 集成套件（dispatch_mock_test.rs，8 项）：axum+rustls TLS mock（自签 rcgen 证书），真 daemon 指 mock，覆盖 cluster_status/vm_list/vm_status/vm_metrics/vm_agent/snapshot list/protected 拒绝/read_only tier
- **产品 bug 由测试套件反抓**：rustls CryptoProvider 不确定性（多 feature 集链接时进程级无 provider）会无声杀死 daemon（空 stderr, exit 101）——PveClient::new 安装 ring provider（幂等）
- 先例借鉴：pnpm executable_link.rs（staging 唯一名+失败自清+rename 重试）、vite-plus windows_path.rs（PATH 注册表安全写：只认 ERROR_FILE_NOT_FOUND、REG_EXPAND_SZ）
