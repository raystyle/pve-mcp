---

paths:

- "crates/**"
- "tests/**"
- "Cargo.toml"
- "Cargo.lock"
- "justfile"
- "clippy.toml"
- "pve-mcp.toml.example"
- "scripts/**"

---

# Rust 开发规则（pve-mcp）

## 架构基线（know-mcp 形态，勿偏离）

- Rust workspace 两 crate：`pve-core`（PVE API 客户端、配置、类型）+ `pve-server`（daemon / MCP / CLI 入口，二进制名 `pve-mcp`）+ `test-support`
- daemon 模式：探测-拉起-等就绪（`ensure_daemon`）；`daemon.lock` 原子 `create_new` 单实例互斥（先于端口绑定）+ 陈旧锁 PID 存活接管；stdio 桥首调用自动拉起 daemon
- 管理面：JSON-per-line over TCP loopback，端口文件（`admin.port` 原子 rename 写入）
- 三消费形态：`pve-mcp stdio`（Claude Code）/ `pve-mcp http`（streamable HTTP）/ CLI 动词
- 配置 `pve-mcp.toml`：`--config` > `PVE_MCP_CONFIG` 环境变量 > exe 同目录 > 工作目录
- Windows 优先 `dunce::canonicalize`（非 UNC 路径）

## daemon 模式战史坑（know-mcp 实测，照抄即可避坑）

| 坑 | 正确做法 |
| --- | --- |
| WMI spawn 不继承环境变量 | `Win32_Process.Create` 时把 `--port-dir`/`--log-level` 显式 echo 进命令行 |
| Git Bash job 会杀子进程 | Windows detached spawn 只信 WMI（DETECTED_PROCESS/CREATE_BREAKAWAY 在 job 禁 breakaway 时照样死） |
| `OpenProcess` 探活把僵尸当活 | 用 `WaitForSingleObject(0)`（进程退出后句柄 signaled）；`OpenProcess` 失败时 `pid == 自己` 算活 |
| probe-then-bind 竞态（双 daemon 互覆端口文件） | 锁文件 `create_new` 必须在绑端口**之前**；输家秒退 |
| 锁目录不存在 → daemon 秒退 → 客户端轮询 300s | 锁是 port dir 第一个产物，先 `create_dir_all` 再 `create_new` |
| 管理面 params 读取错层 | `params` 在 `request.params` 下；top-level 字段只作 legacy 兜底，别只读顶层 |
| 端口文件写一半被读 | 写 `.tmp` 后 `rename` 原子替换 |

## 编码规范

- edition 2021；`thiserror` 用于库 crate 错误类型，`anyhow` 只进二进制（先例 cutweave）；`tracing` 记日志
- `#[serde(deny_unknown_fields, default)]` 配置解析（先例 know-mcp config.rs）——配置字段拼错在加载期报错而非静默默认
- 注释默认无；只在 WHY 非显而易见时写；英文
- 单文件 >1000 行按职责拆分
- 禁 `unwrap()`/`expect()` 于非测试路径（daemon 长驻进程，panic 即死）——锁 poisoned 时报错降级，不 panic
- 项目专属 lint 踩坑后进 `clippy.toml` 的 `disallowed-methods`，每条注明原因，随 clippy 强制（先例 know-mcp）

## 域值品牌类型（默认必用，新旧一律）

- **域值一律 newtype**：`Vmid(u32)`、`NodeName(String)`、`Upid(String)`、`StorageName(String)`、`GuestRef { node, vmid, ty }`、`SnapshotName(String)` 等——裸 `u32`/`String` 传域值是违规（防 primitive obsession；先例 nutype 1782★、Rust Book 官方推荐，研究链见 `.research/dev-system/notes-rust-patterns.md`）
- **构造即校验，内部免检**：校验只在三个边界做 `TryFrom`/`try_new`——MCP 工具参数解码、TOML 配置 parse、PVE API 响应 decode；边界之后整个调用链只传品牌类型，不再重复校验（parse, don't validate）
- **穷尽枚举禁通配臂**：`GuestType`/`PowerAction` 等 `enum` 的 match 必须逐臂写全，`_ =>` 只许用于编译期穷尽守卫（never fallback），不许当业务兜底
- 私有字段 + `#[repr(transparent)]`（零成本）；Display/serde 派生随需
- **选型**：v1 手写 newtype + `TryFrom`（依赖最小）；域值超 ~8 个且校验规则重复时再引入 `nutype`（serde 边界强制校验）
- **反例警示**：运行时动态决定的形态不要硬套类型（先例 TA-Lib：typestate 要求编译期知识）——typestate 只用于编译期可确定的生命周期

## 状态机（生命周期 typestate、运行态枚举 FSM）

- **生命周期用 typestate**：状态编码进类型参数，非法状态编译期不可表示；转换方法消费 `self` 返回新状态类型（先例 rustls ConfigBuilder、Linux 内核 DRM device、iscsi 会话）。本项目落点：daemon 启动序列 `Daemon<Unlocked> → Daemon<Locked> → Daemon<Bound> → Daemon<Ready>`——"锁先于端口绑定"的战史坑直接编码进类型，锁没拿到调不到 `bind`
- **运行态用枚举穷尽 match**：事件驱动转移 + `match` 全臂穷尽，新事件未处理 = 编译错误（XState `assertEvent` 等价）。本项目落点：ticket 会话（未认证→已认证→续签中，2h 过期驱动）、任务跟随（Pending→Running→Finished(exit)/Timeout）
- **线性管道不套状态机**：工具调用流水线（validate → confirm 检查 → execute → audit → respond）是固定顺序的中间件链，不是状态转移——套 FSM 是过度设计
- 复杂层级状态图（需要嵌套/并行/守卫可视化）才引入 statig 类运行时库——本项目状态域均为简单线性图，预期用不到

## 构建与检查门

- 常用命令：`cargo build` / `cargo test -p <crate>` / `cargo clippy --workspace --all-targets -- -D warnings`
- nextest 前置：`cargo install cargo-nextest`；just 前置：`cargo install just`
- 检查门 `just ci` = fmt --check → clippy -D warnings → nextest（建仓时落 justfile；代码动工前先用裸 cargo 命令等价跑）
- workspace lints 统一在根 Cargo.toml `[workspace.lints]`（all=warn、correctness=deny），crate 侧 `[lints] workspace = true`

## 测试纪律

- 单测 `#[cfg(test)]` 模块内联；daemon e2e / 互斥回归 / 冷启动 e2e 放 `crates/pve-server/tests/`
- **测试跑 nextest 而非裸 `cargo test`**：env/cwd 敏感测试在 cargo test 线程模式假红（先例 flockr/know-mcp）
- 测试数非零断言：全绿但 N=0 是失败；新测试先红后绿（破坏被测物 → 看它失败 → 恢复 → 看它绿）
- **不直打生产集群**：对真实 PVE 只做只读冒烟（GET）；写操作测试指向专用测试节点或 mock server（`tests/support/` 可起本地 axum mock PVE API）
- 依赖本机资源（GPU/模型/真实集群）的测试一律 existsSync 探测 + SKIP 并显式报告，不静默通过（先例 vulnforge verify 纪律）

## PVE API 与安全

- **客户端自写，禁引第三方 PVE crate**：`ureq`（json feature）直打 `/api2/json` + `backon` 重试——生态无维护良好的 PVE 客户端库（andor44/rs-proxmox 均弃坑；官方 proxmox-rs 是 PVE 内部工具 crate 非 API 客户端；rash/jetpack/scanopy 全部自写）。研究链：`.research/dev-system/notes-pve-client.md`

### 认证三形态（成熟先例互证，实现单点在 pve-core）

- **Token**：`Authorization: PVEAPIToken=<user@realm!tokenid>=<secret>`；token id 与 secret **两侧都要 trim**；header 构造失败（非法字符）显式报错不 panic；secret 绝不落日志（debug 打 `[SECRET]`，serde_debug 全禁）
- **Ticket**：`POST /api2/json/access/ticket` body **必须 URL 编码**（密码含 `!@` 特殊字符）；响应 `ticket`（蛇形）+ `CSRFPreventionToken`（PascalCase，serde rename）；使用 `Cookie: PVEAuthCookie=<ticket>`，**CSRF header 仅写方法**（GET 不带）；2h 过期前续签，续签失败降级重登录，不 panic
- **SSH**：必须 **root**——PVE 本机非 root 跑 pvesh/qm 结构性关闭（pmxcfs IPC 属 root:www-data，user.cfg 加权救不了，pve-harness M061 实测）；非 root（ray）只能走 https+token

### API 行为纪律（错误与响应语义）

- 响应统一 `{data: T}` 信封；错误响应**有 body**（JSON errors 字段）——错误处理必须解析 body 才能给真实原因，不能只看 HTTP status（官方 proxmox-client 提交教训）
- **401 ≠ 403**：401 = 凭据被拒硬错；403 = 凭据对但无权此路径——只读 token 部分路径 403 是预期，降级窄面不硬错（scanopy 先例）
- 错误分层：连接建立失败 / 请求失败 / 响应解析失败 / TLS 校验失败 / TFARequired 各自变体（官方 proxmox-client 分类学）
- host 尾斜杠剥除；IPv6 字面量加方括号；`/version` 是任何 token 都可读的连通性探测点
- qm/config 批量写**原子性**：一个坏参数全盘否——写失败后必须回读 config 核实实际状态（M015）
- connect_timeout 5s / request_timeout 30s（scanopy 同款基准）

### PVE 环境战史坑（同款环境实测，实现对应工具时必查）

| 坑 | 规则 |
| --- | --- |
| QGA argv ~8KiB 硬顶（b64 超 ~3k 字符必截断） | guest_exec 载荷分片 ≤2800 字符/片，fail-fast 断言（M035） |
| QGA 长载荷超等待窗（回 255，客端仍在跑） | 长载荷发射后轮询收果，不让通道同步等完（M034） |
| pc-q35 无 IDE 总线；sata 变更不热插须重启 | ISO 挂 sata 槽；提示需要重启生效（M040） |
| dir 存储 iso 真实落点 `<path>/template/iso/` | volid 引用形 `isos:iso/<名>.iso`（M040） |
| vmbr1 vlan-aware：无 tag 流量直出真网 | vm_create 校验 vmbr1 必须 tag（M045） |
| pvesh CLI 与 API 面返回语义不同（已解码/未解码） | QGA/lxc 工具走 API 面，不走 pvesh 转发（M021） |
| **WMI 嵌入式参数经 untyped `put_property(Variant::Object)` 静默丢弃**（ReturnValue=0 但内容没到 WMI——黑窗 bug 定论） | WMI 调用一律走 typed API：serde 结构体 + `exec_class_method::<T,_>`（herdr-runtime windows.rs 形态）；wmi ≥0.18 |

### 安全红线

- **凭据红线**：token secret / 用户密码只经环境变量或 gitignored `pve-mcp.toml`；绝不进日志（tracing）、测试 fixture、evidence、仓内文档；`pve-mcp.toml.example` 用占位符
- 工具面破坏性操作（stop/shutdown/reboot/destroy/rollback/migrate/exec）必须 `confirm=true` 参数检查门；CLI 动词必须 `--confirm`；dry-run 默认（先例 pve-harness `create`/`delete` 的 dry-run until --yes 形态）
- MCP 工具返回文本不内嵌 secret；错误信息脱敏（报 401 时不回显凭据）
- 电源动作幂等：先查现态，已达目标态不写（pve-harness power IDEMPOTENT_TARGET 表）
