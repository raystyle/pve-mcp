# pve-client — PVE Rust 客户端库先例研究（2026-11-27）

> 服务 rust-dev.md PVE API 规则固化。检索：gh-mcp 搜"proxmox api client rust"（4 命中）、"PVEAPIToken" code 搜（224 命中取成熟实现）+ 提交史 + issue 史 + pve-harness M102 战史。

## 生态判定：无可用第三方库，自写薄客户端是共识

| 候选 | 判定 | 证据 |
| --- | --- | --- |
| andor44/proxmox-rs、rs-proxmox | ❌ 弃坑 | 2018 / 2022 年后零提交 |
| proxmox/proxmox-rs（官方，49★） | ❌ 不是 API 客户端 | 官方自述"Common Rust Crates"——PVE 内部工具 crate 集（build 依赖 Debian devel repos）；其中 proxmox-client 是 hyper 底座且面向 PVE 自家生态 |
| rash / jetpack / scanopy / FeatherPanel 等成熟项目 | ✅ **全部自写薄客户端** | 224 处 PVEAPIToken 代码命中，无一引第三方 PVE 库 |
| pve-harness（rust 分支） | ✅ 本地血统 | SSH 通道 `sudo -n pvesh` + envelope；M102 战史 12 条 |

**结论：pve-core 自写 ureq 薄客户端（ADR-001 已定），不引依赖**——与生态共识一致。

## 官方 proxmox-client 提交史教训（错误分类学）

| 提交 | 教训 |
| --- | --- |
| `client: only classify connection-establishment errors as connect errors` | 错误分类要按发生阶段：连接建立失败 ≠ 请求失败 ≠ 响应解析失败——pve-core 错误枚举须分层 |
| `client: parse API error response bodies` | PVE 错误响应**有 body**（JSON 含 errors 字段）——不能只看 HTTP status，要解析 body 才能拿到真实失败原因（如"500 错误实际是 pvedaemon 报具体原因"） |
| `client: handle tls verification errors more gracefully` / `clarify error message on mismatching fingerprint` | 自签证书错误要给人话（"证书指纹不匹配"而非裸 tls error） |
| `client: add TfaRequired error variant` | 登录可能被 TFA 拦截——错误类型要有专门变体 |
| `client: detect non-TLS connection failures via hyper-util` | 探测层要区分"连不上"与"连上了但不是 TLS" |

## 成熟实现的认证实现模式（三处互证）

**Token 模式**（scanopy 最严谨，rash/jetpack 同形）：
- header 形 `Authorization: PVEAPIToken=<user@realm!tokenid>=<secret>`（**token id 与 secret 都要 trim**——scanopy 显式 `.trim()`）
- **secret 不落 URL/日志**：scanopy 构造后 `set_sensitive(true)`，debug 日志打 `[SECRET]`（FeatherPanel）
- header 值构造失败（非法字符）是显式错误，不是 panic

**Ticket 模式**（rash/jetpack）：
- `POST /api2/json/access/ticket`，**body 必须 URL 编码**（rash 用 `urlencoding::encode`）——用户名/密码含特殊字符（`@`、`!`）不编码直接拼必炸
- 响应字段是**蛇形 ticket + PascalCase `CSRFPreventionToken`**（serde 要 rename）
- 使用：`Cookie: PVEAuthCookie=<ticket>` + **仅写方法**带 `CSRFPreventionToken` header（GET 不需要 CSRF——rash 显式判 method != GET）
- 2h 过期：jetpack/ry-ops 均在过期前续签

**URL/主机模式**：
- `https://<host>:8006/api2/json` 为基；host 尾斜杠要剥（rash `trim_end_matches('/')`）
- **IPv6 字面量要方括号**（scanopy：`[v6]`）
- 响应是 `{data: T}` 信封——所有调用统一 unwrap 信封

**401/403 语义**（scanopy 最清晰）：
- 401 = token 被拒（凭据错）→ 硬错
- 403 = token 权限不足（凭据对但无权此路径）→ 降级返回 None/窄面，不硬错——**pvebot-ro 只读 token 部分路径 403 是预期行为**

## pve-harness M102 战史（环境同款，直接照抄避坑）

| 编号 | 坑 | 对 pve-mcp 的意义 |
| --- | --- | --- |
| **M061** | PVE 本机非 root（ray）跑 pvesh/qm 结构性关闭（pmxcfs IPC 属 root:www-data）——user.cfg 加权救不了本地 CLI | **SSH 通道必须 root**；ray@lan-pve 只能走 https+token——双通道设计直接定案 |
| M015 | qm 多参数 set 原子性：一个坏全盘否（ide0/1/2 全没挂上） | config 批量写失败后必须复查 config，别信退出码表象 |
| M021 | pvesh CLI agent file-read 已自行解码，proxmoxer ssh 后端塞 errors 键 | QGA 面经 pvesh 的返回语义与 API 面不同——lxc_exec/QGA 工具走 API 而非 pvesh |
| M034 | QGA 长载荷超 qm guest exec 等待窗（回 255） | guest_exec 工具：长载荷用状态文件轮询，不让通道等完 |
| M035 | QGA argv ~8KiB 硬顶（b64 UTF-16LE 超 3k 字符必截断） | guest_exec 载荷 >3k 字符分片（≤2800/片），fail-fast 断言 |
| M040 | pc-q35 无 IDE 总线（ide 槽客端不可见）；sata 变更不热插须重启；dir 存储 iso 真实落点 `<path>/template/iso/` | vm_config 工具：ISO 挂 sata；ISO 路径校验 |
| M045 | vmbr1 是 vlan-aware 桥：无 tag 流量走原生 VLAN 直出真网（实验室直通事故） | vm_create 的 bridge+tag 参数校验：vmbr1 必须 tag |

## 转规则清单（已固化 rust-dev.md）

1. PVE API 客户端自写（ureq + backon 重试），禁引第三方 PVE crate
2. 认证三形态 + 各自硬性纪律（trim、编码、sensitive、方法判别）
3. 错误分层枚举 + body 解析 + 401/403 区分语义
4. M102 战史表进规则（SSH root 通道、QGA 载荷分片、原子性复查、q35/sata、vlan tag）
