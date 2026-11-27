---
name: pve-api
description: PVE API 端点速查与工具实现对照：写新工具（vm_power/vm_create/snapshot 等）或调 API 路径时，查端点形态、参数、长任务 wait 语义、幂等语义。当实现 pve-core 客户端方法、添加 MCP 工具、或调试 PVE API 调用失败时使用。
---

# pve-api — PVE API 端点速查（单节点 v1 工具面对照）

> 端点以 PVE 9.2 为准（实测 lan-pve）。实现新工具前先对照本表；坑细节见 `.claude/rules/rust-dev.md` 战史表与研究链 `.research/dev-system/notes-pve-client.md`。

## 连通与探测

| 端点 | 用途 |
| --- | --- |
| `GET /api2/json/version` | 连通性探测——任何有效 token 均可读（scanopy 同款 connect 校验点） |
| `POST /api2/json/access/ticket` | 密码换 ticket（body URL 编码） |
| `GET /api2/json/` | 根索引（可选） |

## 查询域（8 工具对照）

| 工具 | 端点 | 备注 |
| --- | --- | --- |
| `cluster_status` | `GET /nodes`、`GET /cluster/status` | 单节点 quorum 恒 OK |
| `vm_list` | `GET /cluster/resources?type=vm` | **含 lxc**，按 type 字段分流（pve-harness list 的对拍行数律：qemu 须滤 type==qemu） |
| `vm_status` | `GET /nodes/{node}/{qemu\|lxc}/{vmid}/status/current` + `/config` | running 面另有 `/rrddata` |
| `node_status` | `GET /nodes/{node}/status`、`/disks/list`（SMART） | |
| `task_status` | `GET /nodes/{node}/tasks/{upid}/status` + `?start=0&limit=50` 读 log | UPID 里含 node——不必另传 |
| `storage_list` | `GET /storage`（定义）+ `GET /nodes/{node}/storage`（用量） | 内容清单 `GET /nodes/{node}/storage/{store}/content?content=iso` |
| `vm_metrics` | `GET /nodes/{node}/{type}/{vmid}/rrddata?timeframe={hour\|day\|week\|month\|year}` | node/qemu/lxc 通用 |
| `vm_agent` | `GET .../qemu/{vmid}/agent/get-osinfo`、`agent/interfaces`、`agent/ping` | 仅运行中 + QGA 开；ping 探测 |

## 受控操作域（9 工具对照）

| 工具 | 端点 | 语义要点 |
| --- | --- | --- |
| `vm_power` | `POST .../{type}/{vmid}/status/{start\|shutdown\|stop\|reboot\|reset\|suspend\|resume}` | **幂等**：先 `status/current` 查现态，已达目标态不写（IDEMPOTENT_TARGET：start→running, stop/shutdown→stopped, reboot→running）；shutdown 是 ACPI 温和、stop 硬切（confirm 更严） |
| `vm_create` | `POST /nodes/{node}/qemu/{vmid}/clone`（新 vmid 经 `GET /cluster/nextid` 取） | clone 参数：newid/name/full/storage/target；cloud-init 并发 `POST .../config` |
| `vm_delete` | `DELETE /nodes/{node}/qemu/{vmid}?destroy-unreferenced-disks=1&purge=1` | **仅停机可删**（先查状态）；protected_vmids 拒绝 |
| `vm_config` | `PUT /nodes/{node}/qemu/{vmid}/config` | **原子性**：一个坏参数全盘否（M015），失败后回读 config 核实 |
| `snapshot_manage` | `POST/GET/DELETE .../{type}/{vmid}/snapshot[/{name}]`、rollback: `POST .../snapshot/{name}/rollback` | qemu+lxc 同形；rollback 强制停机语义（API 自动处理）；`current` 不是快照 |
| `backup_manage` | `POST /nodes/{node}/vzdump`、`GET /nodes/{node}/storage/{store}/content?content=backup`、restore: `POST .../qemu` 带 `archive=` | run 返回 UPID |
| `firewall_manage` | `GET/POST/DELETE /nodes/{node}/qemu/{vmid}/firewall/rules` | guest 级规则（集群级不进 v1） |
| `lxc_exec` | SSH root 通道 `pct exec <vmid> -- <cmd>` | 经 SSH 非 API（API 无 pct exec 端点）；载荷大小注意 |
| `vm_agent_exec` | `POST .../qemu/{vmid}/agent/exec` → 返回 PID → 轮询 `agent/exec-status` | **异步两步**（ry-ops 同款）；载荷 >3k 字符分片（M035）；长任务轮询不让通道等完（M034） |

## 长任务 wait 语义（全写操作通用）

1. 写操作返回 `{"data": "UPID:..."}`——非同步结果
2. 轮询 `GET /nodes/{node}/tasks/{upid}/status`：`status: OK` 成功 / `status: <error>` 失败（log 在 tasks/{upid}/log）
3. `wait=true`（默认）：跟随到终态；失败回传 log 尾（最后 ~20 行）
4. 轮询间隔 ~500ms，超时上限配置化（默认 300s；clone 大盘/backup 更长）

## 幂等与状态判定速查

| 动作 | 现态判定 | 已达态 |
| --- | --- | --- |
| start | `status == "running"` | 不写 |
| stop/shutdown | `status == "stopped"` | 不写 |
| reboot/reset | 需 running | 不 running 报错 |
| snapshot create | 同名已存在 | 报错（不覆盖） |
| vm_delete | `status != "stopped"` | 拒绝（提示先停机） |
