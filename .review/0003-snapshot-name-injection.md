# 审查结论：0003 snapshot-name-injection（首轮全量审查）

- 日期：2026-11-27
- 审查对象：main @ 9465d84（crates/ + tools/ + .claude/hooks/ + 配置）
- 审查代理：reviewer（新上下文，规则从 main 读取，未降级）
- 验证：clippy 零告警 / nextest 45/45 / system-lint 10/10 / dot-segment 注入链 URL 归一化实验（url 2.5.9 语义 + ureq 2.12.1 源码确认）

## Blocking（必须修才能 ship）

- **domain.rs SnapshotName 未拒绝 dot-segment；snapshot delete/rollback 名字拼 URL path 不转义** —— 攻击链（已实验证实 URL 规范化行为）：MCP 调 `snapshot_manage {vmid:102, action:"delete", name:"..", confirm:true}` → SnapshotName 校验通过（只拒 empty/current/whitespace/`/`/`,`/`=`）→ 拼 `/nodes/lan-pve/qemu/102/snapshot/..` → ureq `url::Url::parse` WHATWG 归一化 `/snapshot/..` → `/qemu/102/` → 实际 `DELETE /api2/json/nodes/lan-pve/qemu/102` = **销毁整机**（不带 purge 参数、绕过 vm_delete 的 stopped 前置检查与参数面）。用户 confirm 语义是"删快照"，实际删整机。`?` 同样未拒（query 注入向量）。修法：SnapshotName 加 `.`/`..`/`?`/`#`/`%` 拒绝；NodeName 一并收紧（当前 node 名来自 PVE 响应无可达路径，纵深防御）。

## Report（建议修）

1. daemon.rs protected_vmids `get_or_init(unwrap_or_default)` fail-open（当前无可达触发路径——config validate 已在加载期强制 parse；建议改启动期解析存 AdminState，消除兜底分支）
2. wait_task 持全局 client mutex 轮询最长 300s——长任务期间全部工具被 30s 超时阻塞；vm_create/vm_delete/snapshot rollback+delete 缺 `wait=false` 逃生（只有 vm_power 有）
3. snapshot_manage 的 `list`（纯读）走 guest_guard 被 read_only tier 误拒
4. vm_create 的 new_vmid 未对照 protected_vmids（保留段内可建机且此后改删不了）
5. client.rs `ca_bundle` 旋钮是死的（设了不生效，example 文档与行为不一致）；config.rs `server.port_dir` 同样无代码读取
6. api.rs snapshot_list `filter_map(ok())` 静默吞反序列化失败行
7. client.rs classify_transport 丢弃 method/path（连接错误不知哪个请求）
8. controlled-op 的 UPID 校验在写已发出之后（PVE 返回非 9 段时用户收到 error 但写已发生；unconfirmed）
9. CLI 缺 snapshot 动词（docs 声称 1:1 映射，实际 MCP-only）
10. chrono 两 crate 声明零使用——删
11. main.rs set_var 在 tokio runtime 启动后调用（2021 技术上未定义行为；先 set 再建 runtime）
12. system-lint config-example-sync 文件缺失静默过（自称 scaffold 态，违反 SKIP 显式报告纪律）；readdirSync catch→return 同类
13. smoke_test 整文件 cfg 门控：默认跑门下无 SKIP 报告
14. admin plane 非法 JSON 行 continue 静默丢弃（客户端悬到超时）
15. GuestSummary.vmid 边界解码未走品牌类型（无错误行为，规则偏差）
16. admin plane 无鉴权绑 loopback（威胁模型判定可接受：单用户开发机，本地攻击者本可直读 pve-mcp.toml 拿凭据；建议 README 记录该接受的风险）

## Never-report 命中

- ureq/rmcp 选型质疑（规则已定）；daemon_e2e 的 `test-secret` 占位符（9c 白名单）；测试组织/风格偏好；start/resume 免 confirm（NANDI 先例显式声明）

## 安全面评估

- **凭据流向：通过**——密码仅 login body（百分号编码）；手写 Debug 不泄 secret（测试实证）；401/403 不回显凭据；pve-mcp.toml 未跟踪（实测）
- **护栏覆盖：一处 blocking 绕过**（dot-segment 注入）；其余逐项确认（confirm/幂等/dry-run/CLI 双层复检）
- **注入面：snapshot name 是唯一活向量**（vmid 数值、ty 穷尽枚举、node 名来自 PVE 响应；WMI 命令行路径来自 current_exe 非用户输入；admin method 穷尽 match）

## 修复记录

| 项 | 处置 | commit |
| --- | --- | --- |
| **Blocking #1** dot-segment 注入 | SnapshotName 加 `.`/`..`/`./x`/`../x` 拒绝 + `?`/`#`/`%` 拒绝；NodeName/StorageName 同步收紧；新增注入拒绝测试（先红后绿证明）；**真实集群验收**：`delete name=".."` confirm=true 现被边界拒绝（VM 104 完好） | 待提交 |
| Report #3 snapshot list 被 read_only 误拒 | list 改走纯 vmid 解析（不过 guest_guard 写护栏），create/rollback/delete 仍过 guest_guard | 同上 |
| Report #1 fail-open | protected_vmids 改启动期急解析存 AdminState（validate 已保证可解析，消除 unwrap_or_default 兜底）；新增 fail-closed 回归测试（坏配置拒绝启动 + 无端口文件） | 待提交 |
| Report #4 new_vmid 保留段 | vm_create 对 new_vmid 过 protected_vmids span（建在保留段的机将不可管理） | 同上 |
| Report #5 死旋钮 | server.port_dir 从 config + example 移除（无代码读取）；ca_bundle 注释改为明确"预留未实装" | 同上 |
| Report #10 chrono | 两 crate 声明删除（零使用） | 同上 |
| Report #2/#6-#9/#11-#16 | 待后续增量（wait=false 逃生 / 吞行错误 / UPID 后置校验 / snapshot CLI 动词 / set_var 提前 / lint SKIP 显式化 / admin JSON 错误回复 / GuestSummary 品牌 / README 风险声明） | — |
