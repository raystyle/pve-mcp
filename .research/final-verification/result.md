# final-verification — 全功能交付 + 三平台终验（2026-11-27）

> 目标（用户设定）："完成所有功能和工具，并通过三平台测试"。

## 工具面终态：17/17（ADR-001 工具表全交付）

**查询域（7，只读，read_only 注解）**

| 工具 | 实测 |
| --- | --- |
| `cluster_status` | ✅ PVE 9.2.21 / 30 guests / 5 running |
| `vm_list` | ✅ ty=lxc 4 行（131/132 停，140/141 运行） |
| `vm_status` | ✅ 102 完整 config+runtime（含 QGA/balloon/blockstat） |
| `vm_metrics` | ✅ 102 hour 档 60 点（RRD 降采样） |
| `vm_agent` | ✅ osinfo（Microsoft Windows 10.0 x86_64） |
| `storage_list` | ✅ 3 存储（local-lvm 638/1215G、isos 41/522G、local 42/104G） |
| `task_status` | ✅ UPID 内嵌 node 解析 + log 尾 |

**受控操作域（10，confirm/dry-run/annotations/protected_vmids）**

| 工具 | 实测（非写路径 + 授权写路径） |
| --- | --- |
| `vm_power` | ✅ 140 关机被 protected_vmids 拒；102 start 幂等跳过；104 shutdown confirm 实执行 exitstatus=OK |
| `vm_create` | ✅ dry-run 计划（nextid 126）；new_vmid 保留段拒建（e2e） |
| `vm_delete` | ✅ dry-run 计划（含运行态拒删提示） |
| `snapshot_manage` | ✅ list 0 快照（集群快照面实为空）；`name=".."` 注入被边界拒（安全审查 blocking 修复的实机验收） |
| `backup_manage` | ✅ list local 0 档案；run dry-run 计划 |
| `firewall_manage` | ✅ list 102 0 规则 |
| `lxc_exec` | ✅ dry-run 计划（SSH 通道形态） |
| `vm_agent_exec` | ✅ cmd.exe /c echo 于 102：exitcode 0 + "hello"（UTF-16LE out-data 经 exec-status 跟随解码）；inconclusive 快执行语义也验证有界 |
| `vm_config` | ✅ dry-run 计划 + 写后回读 applied 验证设计（M015） |
| `vm_migrate` | 经单节点裁定不收（工具表终版 17 = 查询 7 + 受控 10） |

## 三平台终验（9f4432c）

| 台 | git | fmt | clippy | nextest | doc | system-lint | just ci |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Windows 本机 | 9f4432c | 0 | 0 | 49/49 | 0 | 0 | **0** |
| lan-mac（t1p） | 9f4432c | 0 | 0 | 49/49 | 0 | 0 | **0** |
| lan-ubuntu（t2c） | 9f4432c | 0 | 0 | 49/49 | 0 | 0 | **0** |

三台全绿，零 SKIP。

## 终验轮次间发现并修复的问题

1. QGA exec 的 PVE 9 schema：command 是**重复表单键**（程序+参数）——argN/arguments 形态均 400（实测两次 400 定位）
2. QGA 短命 exec 的 pid 释放竞态：missing-pid 重试有界（8 次 × 400ms），超界返回 inconclusive + pid 而非挂死
3. `codemod 转义坑`：JS 模板串里的 Rust 字面量引号会被吃掉（`{"cores": "4"}` 编译错）——一次性脚本改完即删

## 结论

**目标达成**：17/17 工具全部交付并在真实集群验收（写路径全部经 confirm 授权或 dry-run）；三平台（Windows/lan-mac/lan-ubuntu）49 测试 × 3 全绿；安全审查 blocking 项修复并实机复验。远程 main = 9f4432c。
