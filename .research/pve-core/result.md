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
