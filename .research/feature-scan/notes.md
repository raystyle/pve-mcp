# feature-scan — PVE 需求确立（2026-11-27 实测收口）

> 服务 ADR-001 子需求 3。证据 = 本机 SSH（mesh key root@lan-pve）只读 GET 清点；写操作零执行。

## 集群实况（证据链）

| 项 | 实测 |
| --- | --- |
| 版本 | pve-manager/9.2.21（kernel 7.0.14-24-pve） |
| 拓扑 | 单节点 lan-pve（92 GB mem，cpu 3.6%），非集群 |
| guests | 30（qemu 26 + lxc 4：131/132/140/141，其中 lxc 2 台 running） |
| 存储 | isos（dir: iso/snippets/vztmpl）、local-lvm（lvmthin: images/rootdir）、local（dir: import/backup） |
| 模板/基础机 | win11-ltsc2024-base、srv2025-zhcn-base、srv2025-base、srv2022-zhcn-base、win2019/2016-vol-base、win10-ltsc2021-base、ubuntu-2404-base、win-re-lab-base 等（*-base 命名，克隆源） |
| 业务机 | win-target-01、vulhub-host、wrt-lab、dmz-jump、attack-01、oob-server、rs-writeface-test、GoT 命名组（109-113/116-117）、DMZ lxc 桥（140/141 running） |
| PVE 用户 | root@pam、pvebot-power@pve（HarnessPower+PVEAuditor）、pvebot-ro@pve（PVEAuditor）——**ray 是 Linux SSH 用户，不是 PVE API 用户** |

## 认证形态（实测结论）

- **SSH key 主通道**：`~/.ssh/mesh_id_ed25519`（注释 mesh-lan-self）对 **root@lan-pve 与 ray@lan-pve 均免密**；root 的 authorized_keys 是 `/etc/pve/priv/authorized_keys`（PVE 集群管理，登记 mesh-lan-win/mac/linux 三把）。herdr 机册即用此 mesh key
- `~/.ssh/config` 的 lan-pve 条目（id_ed25519）未被 PVE 收纳——SSH 免密失败根因
- **密码备通道**：root@pam 密码已存 `pve-mcp.toml`（gitignored）；ticket 2h，可自动续签
- **API Token 面已存在**（pve-harness 遗产，可直接复用）：`pvebot-ro@pve!reader`（PVEAuditor，只读）+ `pvebot-power@pve!power`（HarnessPower，电源写面）——最小权限 bot 模式，pve-mcp 直接沿用
- pvebot token secret 不在本机已扫描位置（pve-harness keys 目录未验证）；需要时经 SSH 从 `.keys` 取或经 API 重生成

## pve-harness 26 命令 → MCP 工具映射（草案）

### 收录：只读查询（6 工具）

| MCP 工具 | 对应 harness 命令 | 说明 |
| --- | --- | --- |
| `cluster_status` | nodes + inventory | 节点 + guests + 存储总览 |
| `vm_list` | list | qemu+lxc 清单（过滤 type/status/node） |
| `vm_status` | status + info | 单机 status + config + 运行面（uptime/cpu/mem/net/disk） |
| `node_status` | host | 节点 load/cpu/mem/uptime/磁盘 SMART |
| `task_status` | task | UPID 状态 + 日志尾 |
| `storage_list` | （inventory 内含） | 存储面清单 |

### 收录：受控操作（confirm 闸门 + dry-run 默认）

| MCP 工具 | 对应 harness 命令 | 护栏 |
| --- | --- | --- |
| `vm_power`（action: start/shutdown/stop/reboot/reset） | power + start/stop/shutdown/reboot/reset 别名 | stop/reset 硬切电 = 应急闸门（PVE_ALLOW_STOP 形态）；shutdown ACPI 优先 |
| `vm_create` | create | 克隆模板；dry-run 默认，`confirm=true` 执行 |
| `vm_delete` | delete | 仅停机可删 + 清盘；dry-run 默认 |
| `snapshot_manage`（action: create/list/rollback/delete） | （harness 无，PVE 原生面） | rollback/delete 需 confirm |
| `vm_migrate` | （harness 无） | 需 confirm；单节点集群实际少用，v2 再议 |
| `lxc_power`（action: start/stop/shutdown） | （harness 无 pct 面） | stop 需 confirm；集群有 4 台 lxc 在用 |

### 不收录（靶场专属域，明确划界）

vulhub（靶场编排）、detect（遥测面）、cvrf（MSRC 公告）、attend（无人值守 ISO）、resources（资产分发）、deploy（OOBE shepherd）、template bake、channels/guest/console（rmux 客户机通道，安全面另议）、screen+OCR（screendump 可后补为只读工具）

### 认证后端取舍（需求裁定项，待用户确认）

| 方案 | 内容 |
| --- | --- |
| A（推荐） | API 主通道：pvebot-ro（读）+ pvebot-power（写）双 token 分权 + SSH mesh key 兜底（pveum/紧急面） |
| B | SSH 主通道（pve-harness 形态，root 全权）+ API 备用 |

## 未决项

1. 工具收录表与认证方案 A/B → 用户确认后写入 ADR-001
2. pvebot token secret 获取方式（取现有 or 重生成）
3. guest_exec（SSH 客户机执行面）v1 是否收录——高危，建议 v2
