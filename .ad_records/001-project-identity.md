# ADR-001 | 项目定位：pve-mcp

- 日期：2026-11-27
- 状态：实施中（2026-11-27 用户批准："开始"）

## 需求

用户原话："我想参考 D:/repos/know-mcp 的架构做一个 pve 虚拟机平台的 mcp 程序项目"。定位裁定（2026-11-27）：**"我们把单 pve 功能发挥到极致"**——单节点 PVE 深度运营面，不做多集群管理器。

### 子需求

1. **架构参考 know-mcp**：Rust workspace（pve-core + pve-server + test-support）、rmcp 官方 SDK、daemon 探测-拉起-等就绪模式、锁文件单实例、stdio + HTTP 双传输、CLI 动词、TOML 配置
   - **类型安全机制**（用户裁定 2026-11-27，先例研究收口）：域值 newtype 品牌类型（构造即校验，三边界 TryFrom）+ 生命周期 typestate（daemon 启动序列状态进类型）+ 运行态枚举穷尽 FSM（ticket 会话/任务跟随）；规则已固化 `.claude/rules/rust-dev.md`
2. **功能面 = 单节点 PVE 全能力**（先例研究收口：ry-ops 346 / NANDI 147 / gilby 40 / chajus1 70+ 的单节点子集，聚合动作不学工具数失控）
3. **认证双通道**：SSH mesh key（root@lan-pve，herdr 同款）+ API token（复用 pvebot-ro/pvebot-power 分权）+ 密码 ticket 备用（已存 pve-mcp.toml，gitignored）
4. **护栏体系**（四先例共同模式全采纳）：confirm=true 结构化错误、MCP annotations、`anthropic/requiresUserInteraction` 人审、protected_vmids、access_tier 分权、审计日志、wait_for_task 语义、dry-run 默认
5. **破坏性操作不裸奔**：任何写面工具必须有 confirm 闸门与审计记录

### 工具表（v1 终版草案，17 工具）

**查询域（8，只读）**

| 工具 | 面 |
| --- | --- |
| `cluster_status` | 节点/版本/quorum/资源总览 |
| `vm_list` | 全 guest 清单（qemu+lxc；过滤 type/status/node/template） |
| `vm_status` | 单 guest：status + config + 运行面 + QGA IP |
| `node_status` | 负载/CPU/内存/uptime + 磁盘 + SMART + 网桥清单 |
| `task_status` | UPID 状态 + log 尾（`wait` 参数跟随长任务） |
| `storage_list` | 存储定义/用量/内容（ISO/vztmpl 清单并入） |
| `vm_metrics` | RRD 数据（对象 × timeframe 参数化，不学 NANDI 15 工具） |
| `vm_agent` | QGA 只读：ping/osinfo/fsinfo/network-interfaces（IP 发现） |

**受控操作域（9，confirm + annotations + 人审）**

| 工具 | 护栏 |
| --- | --- |
| `vm_power`（start/shutdown/stop/reboot/reset/suspend/resume） | stop/reset 应急闸门；wait 默认 |
| `vm_create` | 模板克隆 + cloud-init 全参；dry-run 默认 → confirm 执行 |
| `vm_delete` | 仅停机可删 + 清盘；dry-run 默认；protected_vmids 拒绝 |
| `vm_config`（resize_disk/set_cloudinit/update） | confirm |
| `snapshot_manage`（create/list/rollback/delete） | rollback/delete confirm；qemu+lxc |
| `backup_manage`（run/list/restore） | vzdump；restore confirm；run 带 wait |
| `firewall_manage`（guest 级规则 list/create/delete） | 写动作 confirm；靶场网络分段刚需 |
| `lxc_exec` | pct exec 经 SSH 通道；confirm；输出截断 |
| `vm_agent_exec` | QGA 异步 exec（PID→status 模式）；confirm |

**明确不进 v1**（单节点无此面或属靶场专属域）：多集群/PDM、HA/replication、Ceph（节点无）、SDN、网络设备写面（bridge CRUD 可断管理口，永后置）、access 权限管理写面（pvebot 双 token 已固化）、deploy recipes（pve-harness template/attend 域）、ISO 下载（v2 视需求）。

## 进度状态

| 子需求 | 状态 | 剩余 |
| --- | --- | --- |
| 1 架构基线 | 未开始 | workspace 脚手架 |
| 2 工具表 | ✅ 终版草案（本 ADR） | 用户终审 |
| 3 认证双通道 | ✅ 环境已实测（mesh key root/ray 免密；pvebot token ACL 在位） | token secret 接入 |
| 4 护栏体系 | ✅ 设计定稿（先例驱动） | 实现随工具面 |
| 5 破坏性操作闸门 | 设计含于 4 | 实现验证 |

## 最终产物

（交付时回填）

## 是否满足需求

（验证后回填）
