# rust-patterns — 品牌类型与状态机先例研究（2026-11-27）

> 服务 ADR-001 架构裁定与 rust-dev.md 规则固化。检索：gh-mcp typestate/newtype/statemachines。

## 检索命中（证据链）

| 先例 | 证据 | 用法 |
| --- | --- | --- |
| **nutype**（greyblake，1782★） | README 全读 | newtype + sanitize/validate 宏；**serde 反序列化也强制校验**（`::try_new` 是唯一合法构造路径，非法值无法实例化）——"parse, don't validate" 的成熟实现 |
| **rustls builder**（catboost 内嵌 rustls-0.23.46 builder.rs） | 源码注释原文 | `ConfigBuilder` 用 typestate 保证"每个必需配置项恰好提供一次"，State 类型参数跟踪——TLS 库生产级 typestate 先例 |
| **Linux kernel Rust DRM device**（beagleboard/linux rust/kernel/drm/device.rs） | 源码注释原文 | `Device<T, Registered>`——内核级多阶段设备注册 typestate（DeviceContext trait），每个阶段一个类型 |
| **iscsi-crate typestate_session** | 模块文档 | RFC 3720 iSCSI 会话状态图完整编码进类型，"非法转移 = 编译错误而非运行时错误"——协议状态机 typestate 先例 |
| **chromium process_state.rs** | 源码注释 | 变更 API 只挂在 MutableProcessState 类型包装上——typestate 守护进程级不变量 |
| **statig**（814★） | 仓描述 | 层级事件驱动状态机运行时库（hierarchical, event-driven）——运行态 FSM 的库方案 |
| type-lib / branded / plusthree / xstate 端口 | 搜索命中 | 补充生态；plusthree 为 XState v6 端口（用户贴文佐证） |

## 结论（与用户贴文互证）

1. **newtype 是 Rust 官方推荐等价物**（Rust Book / Comprehensive Rust / rust-unofficial patterns），名义类型系统天然防 primitive obsession，零成本（`#[repr(transparent)]`）
2. **校验型 newtype 有成熟库**：nutype 把"构造即校验（含 serde 边界）"做成了宏——正好覆盖 MCP 工具参数、TOML 配置、API 响应三个边界
3. **typestate 有重量级生产先例**：rustls（TLS 配置）、Linux 内核（DRM 设备注册）、iscsi（协议会话）——daemon 启动序列（锁→绑定→就绪）同构
4. **运行态 FSM**：枚举穷尽 match 是默认；复杂层级图才上 statig/XState 端口库——pve-mcp 状态域（ticket 会话/任务跟随）都是简单线性图，枚举足够
5. **反例警示**（TA-Lib 命中注释）：运行时动态场景硬套 typestate/const generics 会要求编译期知识——typestate 只用于编译期可确定的生命周期

## 规则候选（待用户确认后写入 rust-dev.md）

1. **域值 newtype 默认必用**：`Vmid`/`NodeName`/`Upid`/`StorageName`/`GuestRef` 等域值一律 newtype + 边界 `TryFrom`（MCP 参数解码 / TOML 配置 parse / API 响应 decode 三边界校验，内部免检）；裸 `u32`/`String` 传域值违规；穷尽枚举禁通配臂（`_ =>` 只许编译期守卫用）
2. **生命周期 typestate、运行态枚举 FSM**：daemon 启动序列（Unlocked→Locked→Bound→Ready）状态编码进类型参数（rustls builder 先例）；ticket 会话/任务跟随用枚举 + 穷尽 match；线性管道（工具调用 validate→confirm→execute→audit）不套状态机
3. **校验宏选型**：v1 手写 newtype + TryFrom（依赖最小，know-mcp 形态一致）；域值超过 ~8 个且校验规则重复时引入 nutype（serde 边界强制校验）
