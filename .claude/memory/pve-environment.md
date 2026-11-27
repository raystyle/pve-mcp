---
name: pve-environment
description: PVE 集群环境事实（地址、主机名、用户、认证方式）与远程仓
metadata:
  type: reference
---

# PVE 集群环境

- **PVE Web/API**：https://192.168.88.88:8006（自签证书，客户端需 `--insecure`/忽略 cert 校验）；2026-11-27 实测可达（未认证 GET /api2/json/version 返回 401，3s 响应）
- **备用路径**：Tailscale 也可达（tailscale0 inet 10.10.10.7）——公网/异地经 tailscale 网连 PVE
- **集群规模（2026-11-27 大清理后）**：8 guest = vulhub-host(106) + 7 模板；protected_vmids=106
- **主机名**：lan-pve（SSH alias）
- **用户**：root 与 ray；**无密码，SSH key 免密**——PVE API 的用户名密码 ticket 登录路径不可用（无密码）；API Token 可经 SSH `pveum user token add` 创建
- **认证含义（ADR-001 修订）**：认证面 = SSH key 直连（跑 pvesh/qm/pct）+ API Token（经 SSH 创建）；原"用户名+密码 ticket 双支持"需求需用户裁定改为"SSH 通道 + API Token 双支持"或保持两支持但密码项仅留接口

# 远程仓

- **github.com/raystyle/pve-mcp**：PUBLIC、空仓（2026-11-27 建）——本项目 git remote 预留

关联：[[work-state]] [[decisions-pending]]
