# pve-mcp

> Proxmox VE 虚拟化平台 MCP server：对单节点 PVE 集群做查询与受控管理的本地 daemon 服务。架构参考 know-mcp，功能面对标社区先例（ry-ops/NANDI），护栏体系全采纳。

## 快速开始（开发）

```bash
cargo build --workspace
just ci          # fmt → clippy -D warnings → nextest + doc tests
```

## 状态

脚手架期：pve-core 域类型（品牌 newtype）+ 配置解析已落地并全门绿；daemon / MCP 工具面 / PVE 客户端按 ADR-001 工具表推进（见 `.ad_records/001-project-identity.md`）。

## 结构

```
crates/pve-core      # 域类型（品牌 newtype）+ 配置 + PVE API 客户端（自写 ureq）
crates/pve-server    # daemon + MCP（rmcp）+ CLI 入口
crates/test-support  # 测试公用
```

连接配置：`pve-mcp.toml`（gitignored；模板见 `pve-mcp.toml.example`）。

开发体系（Claude Code）：`.claude/`（rules / skills / hooks / memory），审查规则 `REVIEW.md`，决策 `.ad_records/`。
