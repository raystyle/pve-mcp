# PVE MCP 先例功能矩阵（2026-11-27 gh-mcp 研究收口）

> 服务 ADR-001 工具表裁定。检索路径：search_repositories "proxmox mcp server"（169 命中，取 4 个代表）+ TOOLS.md/README 通读 + 工具模块树清点。

## 先例概览

| 先例 | 栈 | 工具数 | 定位与特色 |
| --- | --- | --- | --- |
| **ry-ops/proxmox-mcp-server**（110★） | Python | **346**+89(PDM) | 全 API 面：14 域 + Datacenter Manager 跨集群；deploy recipes（ubuntu/kali/win11/fortigate）；生态最成熟 |
| **NANDI-Services/proxmox-mcp** | TS | **147** | 生产护栏最深：三档 tier + confirm + `requiresUserInteraction` 元数据 + 工具黑白名单 |
| **gilby125/mcp-proxmox**（56★） | Node | ~40 | 权限分级（默认只读 + ALLOW_ELEVATED）+ node/VMID allowlist + Terraform 导出 |
| **chajus1/proxmox-mcp-enhanced** | Python | **70+** | 靶场/安全实验室向（CyberLabSetup/LabCleanup）——与我们场景最接近 |

## 完整功能域矩阵（四先例并集）

| 域 | 工具面（并集） | ry-ops | NANDI | 我们 v1 取舍 |
| --- | --- | --- | --- | --- |
| **集群/节点查询** | nodes/status/resources/options/log/version/quorum/HA/replication | ✓37 | ✓8 | ✅ 收（cluster_status/vm_list/vm_status/node_status/task_status/storage_list） |
| **QEMU 生命周期** | start/shutdown/stop/reboot/reset/suspend/resume | ✓46 | ✓22 | ✅ 收（vm_power 聚合；suspend/resume 可并） |
| **QEMU 管理** | create/clone/delete/migrate/config 更新/磁盘扩容-move/转模板 | ✓ | ✓ | ✅ 收 create(clone)/delete；❌ config 原子改/磁盘 move/转模板（v2） |
| **LXC** | 同 QEMU 全套 + pct exec | ✓28 | ✓18 | ✅ 收 power/create/delete（140/141 在跑）；❌ v2：lxc_clone/config |
| **快照** | create/list/rollback/delete（qemu+lxc） | ✓ | ✓ | ✅ 收 snapshot_manage（集群快照面空白=新价值） |
| **任务** | list/status/**log**/stop + **wait_for_task** | ✓ | ✓4 | ✅ 收 task_status（含 log 尾）；**wait 语义必收**（clone/delete 等长任务 ry-ops 全带 wait:true） |
| **guest agent（QGA）** | **agent_exec/status**（返回 PID→退出码+输出）、agent_info/network/osinfo/fsinfo、IP 发现 | ✓ | ✓ | ⚠️ **新裁定项**：read 面（osinfo/fsinfo/IP）建议收；exec 面标 destructive+confirm（v1 或 v2） |
| **存储** | 定义/内容/卷/状态/create/update/delete | ✓14 | ✓8 | ✅ 只读收；❌ 存储管理写面（单节点 local-lvm 无需） |
| **备份** | vzdump run/restore/backup job CRUD | ✓ | ✓8 | ❌ v2（local dir 备份存在但非高频） |
| **监控 metrics** | RRD 数据 hour/day/week/month/year（node/qemu/lxc）+ syslog + services | ✓ | ✓27 | ⚠️ 可选：`vm_metrics`（RRD 单工具带 timeframe 参数，不学 NANDI 15 个工具） |
| **网络管理** | bridge/bond/SDN CRUD | ✓16 | ✓8 | ❌ 不收（vmbr1 六段实验网手工管；写面高危） |
| **防火墙** | 集群/节点/guest 规则 CRUD + aliases/ipsets/macros/log | ✓31 | ✓12 | ❌ v2+（靶场防火墙由实验自己管） |
| **权限/access** | user/token/group/role/ACL/realm CRUD | ✓33 | ✓13 | ❌ 不收（pvebot 双 token 已固化，不动态改权限） |
| **磁盘/Ceph/ACME/SDN/通知** | SMART/wipe/ZFS/Ceph 全套/证书/通知端点 | ✓ | — | ❌ 不收（单节点无 Ceph；SMART 并入 node_status） |
| **pools** | CRUD | ✓5 | ✓5 | ❌（集群 pools 为空） |
| **templates/cloud-init** | 模板列表/ISO 列表/cloudinit dump/**set** | ✓ | ✓6 | ✅ 收只读（vm_list 已含 template 标志）；cloudinit set 并入 vm_create 参数 |
| **remote/SSH** | pct exec、host diagnostics、docker in container、任意容器命令 | ✓ | ✓6 | ❌ v2（SSH 兜底通道已存在于 CLI 面） |
| **deploy recipes** | ubuntu/kali/win11/fortigate/PDM 一键部署 | ✓ | — | ❌ 不收（= pve-harness template/attend 靶场域，已划界） |
| **PDM 跨集群** | 89 工具经 Datacenter Manager | ✓ | — | ❌（单节点） |

## 四先例共同的安全模式（全部采纳）

| 模式 | 做法 | 采纳 |
| --- | --- | --- |
| **confirm=true** | 破坏性操作无 confirm 返回结构化错误 `CONFIRMATION_REQUIRED`（NANDI）；缺 confirm 报错而非静默执行 | ✅ 已在表（工具返回结构化错误码，不静默） |
| **readOnlyHint/destructiveHint 注解** | MCP 标准 tool annotations，客户端可据此自动批读/人审写（ry-ops） | ✅ 新增收（rmcp 支持 annotations） |
| **`anthropic/requiresUserInteraction`** | 47 个 gated 工具声明 `_meta`，Claude Code ≥2.1.199 每次调用强制人审、allow 规则不可豁免（NANDI） | ✅ 新增收（对 Claude Code 侧加固，与 confirm 双保险） |
| **access tier** | read-only / read-execute / full 三档（NANDI）；PROXMOX_READ_ONLY+NO_DESTRUCTIVE 双开关（ry-ops） | ✅ 简化收：配置 `access_tier = read_only | full`（映射 pvebot-ro/pvebot-power 双 token） |
| **protected VMIDs** | ID/区间名单禁改（ry-ops PROXMOX_PROTECTED_VMIDS；gilby allowlist+protection-flag） | ✅ 新增收：`protected_vmids` 配置（首推 140/141/106/114 在跑设施） |
| **wait_for_task** | 长任务（clone/migrate/backup）带 wait 参数 + 失败回传 log；独立 wait_for_task 工具 | ✅ 收：任务型工具带 `wait=true`（默认）+ 失败附 log 尾 |
| **审计日志** | 每次写调用一行 JSON：who/what/outcome/task，密钥脱敏（ry-ops） | ✅ 新增收：daemon 写审计日志（与凭据红线呼应） |
| **token 最小权限** | PVEAuditor 只读 + 分权写 token（ry-ops/gilby 同荐） | ✅ 已定（pvebot 双 token 复用） |
| **ticket 自动续签** | 密码模式 2h 过期前续（ry-ops） | ✅ 已定（密码备通道） |

## 先例教训

1. **工具数失控是反面教材**：ry-ops 346 个工具烧上下文，被迫发明 PROXMOX_TOOLSETS 裁剪（346→127）；NANDI 用 category 过滤。**我们 v1 目标 ≤15 个工具**，聚合动作（vm_power/snapshot_manage 用 action 参数）而非一动作一工具
2. **NANDI 对 confirm 的清醒认知**：confirm 由 agent 自己传，"防的是事故不是自信的 agent"——所以要叠加 requiresUserInteraction 让**人**批准。双层护栏设计直接照抄
3. **ry-ops 的 guest agent exec 语义**：exec 返回 PID（异步）→ status 查退出码+输出；不是同步 shell——QGA 通道天然比 SSH 安全（只进开了 agent 的客户机）
4. **NANDI 文档同步纪律**：TOOLS.md 由 CI 再生成，drift 即 build fail——我们的工具表也应代码驱动生成（注册表投影，先例 pve-harness `--llms` 同款）
5. **chajus1 的 lab_tools 证实**：靶场域（CyberLab setup/cleanup）确实有 MCP 化先例，但那是 pve-harness 的地盘，维持划界

## 对工具表草案的修订（待用户确认）

新增 4 项（先例驱动）：
1. `vm_agent`（action: info/osinfo/fsinfo/network-ip）——QGA 只读面
2. 全部写工具带 MCP annotations（readOnlyHint=false + destructiveHint）+ `_meta requiresUserInteraction`
3. 配置面新增 `protected_vmids` + `access_tier`；daemon 审计日志
4. `vm_metrics`（可选，RRD timeframe 参数化单工具）

维持：6 只读 + 6 受控；deploy/firewall/access/network-admin/ceph/backup 域不进 v1。
