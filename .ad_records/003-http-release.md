# ADR-003 | HTTP 传输 + v0.2.0 改进积攒

- 日期：2026-11-28
- 状态：实施中

## 需求

v0.1.0 封版后进入改进积攒期：不再逐项立即封版，功能与修复攒到一批后打 `v0.2.0`。用户指令："这个功能再积攒一些改进准备放到下一个版本封版"（指 HTTP MCP 传输及其伴生改进）。

### 子需求

1. **HTTP MCP 传输**：Streamable HTTP（`pve-mcp http`，/mcp 端点），stdio/HTTP 双传输——✅ 交付
2. **know-mcp 结构对齐**：stdio/http 均 ensure_daemon 先行（首调用不在 initialize 内付冷启动）——✅ 交付
3. **token ACL 修正**：PVEAuditor 授 / 根（/nodes+/vms 作用域下 /cluster/resources 静默空）——✅ 集群侧修复 + 坑进战史表
4. **README 补齐**：安装部署指南、token 建法、MCP 接入、CLI 用法 + GitHub 仓库描述——✅ 交付
5. **积攒中**：后续改进继续入此 ADR 的进度表，攒批后 v0.2.0

## 进度状态

| 子需求 | 状态 | 证据 |
| --- | --- | --- |
| 1 HTTP 传输 | ✅ 交付 | http.rs；三步握手 + tools/list 16 工具 + tools/call 真数据实测 |
| 2 结构对齐 | ✅ 交付 | stdio/http 两入口 ensure_daemon 先行 |
| 3 token ACL | ✅ 交付 | pvebot-ro@pve!mcp2 授 / 根；坑进 rust-dev.md 战史表 + toml 注释 |
| 4 README/描述 | ✅ 交付 | 安装/token/MCP 接入/CLI 全链指南；仓库描述已设 |
| 5 v0.2.0 封版 | 🔲 积攒中 | 触发条件：HTTP 使用反馈 + 改进批成型 |

## 积攒清单（v0.2.0 候选，随用随记）

| 候选项 | 来源 | 状态 |
| --- | --- | --- |
| 写面 dispatch arms 集成测试补齐（backup/firewall/power 写路径） | mock 套件已有 403 端点 | 候选 |
| ACL 静默空的读面上报改进（403 与空列表区分） | token ACL 坑 | 候选 |
| HTTP 传输认证（当前 127.0.0.1 无鉴权，绑定非回环需加） | NANDI 先例 | 候选 |
| CHANGELOG.md（Keep a Changelog 形态，v0.1.0 + v0.2.0 双节） | 发布规范 | 候选 |

## 最终产物

| 产物 | 路径 | 一句话 |
| --- | --- | --- |
| HTTP 传输 | crates/pve-server/src/http.rs | StreamableHttpService + LocalSessionManager |
| 对齐 | crates/pve-server/src/mcp.rs + main.rs | ensure_daemon 先于 MCP 握手 |
| ACL 坑 | rust-dev.md 战史表 + pve-mcp.toml 注释 | PVEAuditor 授 / 根 |
| README | README.md | 安装/token/接入/CLI 全链指南 |
| 仓库描述 | GitHub repo description | 17 工具 + 护栏 + token + 自部署 |

## 是否满足需求

（v0.2.0 封版时回填）
