# .research/ — 研究知识库

**定位**：先例、验证、证据、结论——**不含 plan**（计划是执行安排，归 `.claude/plans/` 与 `.ad_records/`）。每个 feature 一个自洽目录；进 git；`evidence/` 只留关键文件。

## 目录结构（规范）

```
.research/
└── <feature>/
    ├── notes.md            # 研究笔记：先例发现、坑、结论（/precedent 产出）
    ├── <专题研究>.md        # 深入研究（API 行为研究、性能分析等，随需增补）
    ├── evidence/           # 验证证据：测试 verbose 日志、先红后绿记录、冒烟输出
    └── result.md           # 结论：跑了什么、退出码、测试数、审查结论、回路记录
```

## feature 索引

| feature | 内容 | 关联 ADR |
| --- | --- | --- |
| [dev-system](dev-system/) | Rust Claude 开发体系先例（tapo/keeler/#34132）、rust 模式、PVE 客户端生态 | ADR-002 |
| [feature-scan](feature-scan/) | 集群清点、PVE MCP 先例工具矩阵、工具表 | ADR-001 |
| [pve-core](pve-core/) | 域类型 + 配置 + API 客户端首增量验证 | ADR-001 |
| [peer-round](peer-round/) | 三平台联动验证（lan-mac/lan-ubuntu 常驻 peer） | ADR-002 |
| [final-verification](final-verification/) | 17/17 工具终验 + 三平台全绿（9f4432c） | ADR-001 |

## 与 plan / ADR 的分工对齐

| 文档         | 回答的问题                                       | 归属   |
| ------------ | ------------------------------------------------ | ------ |
| **plan**     | "将要做什么、怎么做、分几步"——执行安排           | 计划域 |
| **research** | "先例怎么做、我们验证了什么、证据是什么"——知识与事实 | 研究域 |
| **ADR**      | "用户决定了什么、需求满足没有"——决策与验收       | 决策域 |

对齐规则：

- research 的**结论**（result.md）可回溯到需求条目（feature 名 ↔ ADR 编号）
- research **不重复 plan 内容**：实现步骤、批次安排属计划域
- research **不预设决策**：ADR 是用户决定；research 只提供决策依据，结论措辞以"实测表明/先例显示"开头
- evidence 与日志**脱敏后落盘**：凭据、主机名、token 一律不进 evidence（凭据红线）
