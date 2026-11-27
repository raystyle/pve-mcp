---
name: interaction-patterns
description: 用户交互修正模式（自省提炼，观察级未晋级 rule）
metadata:
  type: feedback
---

# 交互修正观察

- **用户同一句重发（≥2 次）= 首答没切中提问意图**。实例：问"routeos 要不要做设置才能访问 96.x"（问的是路由器侧），首答讲成了 PVE 侧路由原理，用户原句重发一遍。
- **Why**：回答前没确认问题主体是哪个设备/哪一层。
- **How to apply**：检测到同句重发时，先用一句话确认"你问的是 X 侧对吧"再展开；网络/拓扑类问题先分清提问主体（路由器/PVE/容器）再作答。

关联：[[work-state]]
