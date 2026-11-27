# lan-pve 集群清点（2026-11-27，全只读）

> 证据链：本机 SSH（mesh key root@lan-pve）只读 GET；写操作零执行。服务 ADR-001 需求底座。

## 硬件

| 项 | 值 |
| --- | --- |
| 机型 | MINISFORUM MS-A2（F1WSA）工作站 |
| CPU | AMD Ryzen 9 8945HX（32 线程，单路，最高 5.06 GHz，Radeon iGPU） |
| 内存 | 92 GiB 可用（96 GB 物理）+ 8 GB swap；实测 40/92 GiB 在用 |
| 磁盘 | 2× Samsung NVMe 1TB（980 PRO + 980），SMART 均 PASSED |
| LVM | VG `pve` 1.82 TB 全量划入 thin pool（0 free——扩容前只能靠 thin 超分） |

## 软件 / 网络

| 项 | 值 |
| --- | --- |
| PVE | pve-manager 9.2.21，kernel 7.0.14-24-pve，单节点（无集群） |
| 管理 IP | vmbr0 = 192.168.88.88/24（LAN）；Tailscale 10.10.10.7 |
| 实验网 | vmbr1 独立桥 + VLAN 子接口 .90/.95/.96/.97/.98/.99（各 192.168.x.1/24 网关）——靶场分段（DMZ 等）由 vmbr1 承载 |
| 网口 | nic0/nic1 UP（vmbr0/vmbr1 各一），nic2/3/4 + WiFi DOWN |

## 存储

| 存储 | 类型 | 总量 | 已用 | 内容 |
| --- | --- | --- | --- | --- |
| local-lvm | lvmthin | 1215 GB | 639 GB（53%） | 客户机盘（images/rootdir） |
| isos | dir | 522 GB | 41 GB | iso/snippets/vztmpl |
| local | dir | 105 GB | 43 GB | backup/import |

## QEMU 实例（26 = 7 模板 + 19 实例）

**模板（template 标志位，克隆源）**

| vmid | 名称 | mem | disk |
| --- | --- | --- | --- |
| 100 | ubuntu-2404-base | 4G | 40G |
| 101 | win11-ltsc2024-base | 8G | 80G（多盘：efi+tpm+主盘） |
| 103 | win2019-vol-base | 4G | 40G |
| 105 | srv2022-zhcn-base | 4G | 40G |
| 107 | win2016-vol-base | 4G | 40G |
| 120 | win10-ltsc2021-base | 4G | 60G |
| 121 | srv2025-base | 4G | 40G |

**实例（运行 3 台：102 win-target-01、106 vulhub-host(32G/800G)、114 wrt-lab）**

- Windows 攻防面：102 win-target-01（running）、108 win-re-target-01、115 win-re-debug-01、123 attack-01、124 oob-server、122 dmz-jump
- GOAD/维斯特洛组：109 kingslanding、110 winterfell、111 castelblack、112 meereen、113 braavos、116 changan、117 luoyang
- 其他：104 prs-listener、106 vulhub-host、114 wrt-lab、125 rs-writeface-test
- **伪模板**（*-base 命名但无 template 位，是普通停机 VM）：118 srv2025-zhcn-base、119 win-re-lab-base

## LXC（4 台，运行 2：140/141）

| vmid | 名称 | 状态 | mem/disk |
| --- | --- | --- | --- |
| 140 | dmz1-bridge | running | 0.5G/4G |
| 141 | dmz2-relay | running | 0.5G/4G |
| 131 | php2s-deb-snap | stopped | 1G/8G |
| 132 | php2s-ubuntu | stopped | 1G/8G |

## ISO / 容器模板库

- **ISO**：Win Svr 2016/2019/2022/2025、Win11 LTSC 2024、Win10 LTSC 2021（中文版为主）、ubuntu-24.04 cloudimg、virtio-win、GOAD files、RE KB 补丁包（202608）、mssql-2019-express、attend-win2019
- **vztmpl**：debian-12-standard、ubuntu-24.04-standard
- **快照**：抽查 100/101/102 均无快照——快照面未被使用，`snapshot_manage` 工具将是新能力

## 配置样例（102，running）

qemu-guest-agent 开、virtio-scsi-single、win11 ostype、vmbr0 桥——克隆链路成熟（template → clone → cloudinit/QGA）

## 对需求的含义

1. **LXC 必须一等公民**：DMZ 桥（140/141）是运行中的网络设施，只管 qemu 会砍掉活动面
2. **模板克隆是主创建路径**：7 个模板 + cloudinit/QGA 成熟；`vm_create`（clone）比 `vm_new`（从零装）更贴实际
3. **快照面空白**：`snapshot_manage` 提供新价值（回滚实验机）
4. **存储过半**：local-lvm 53% 用量且 thin pool 0 free，`vm_delete` 清盘能力有实际运维价值
5. **vmbr1 VLAN 分段**：克隆时网桥选择（vmbr0 vs vmbr1）是 `vm_create` 参数设计点
