---
name: decisions-pending
description: 待用户确认的决策项（ADR 草案、git init 等）
metadata:
  type: project
---

# 决策待确认

1. **ADR-001（项目定位）草案**：`.ad_records/001-project-identity.md` 状态"提案中"——内容来自用户会话原话与裁定（参考 know-mcp 架构 / daemon 保留 / 认证双支持 / 功能面参考 pve-harness），待确认后改"实施中"
2. **ADR-002（开发体系）**：直接执行令，状态"实施中"；满足性验证后回填
3. **git init 时机**：体系已建，init 后首 commit 建 main（reviewer 正本切 main 读取）；待用户决定
4. **功能面清点**：pve-harness 26 命令 REGISTRY → MCP 工具表映射（`.research/feature-scan/`），动工前完成

关联：[[work-state]]
