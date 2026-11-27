# peer-round — 三平台联动验证首战（2026-11-27）

> feature: 三平台 peer 轮体系建立（ADR-002 子需求延续，用户裁定：学习 vulnforge 建 lan-mac/lan-ubuntu 常驻会话联动测试）。

## Round 0（门轮建立）

| 台 | git | 结果 |
| --- | --- | --- |
| Windows 本机 | 76e7528 | 门绿（但 unix 分支零编译——结构性盲区） |
| lan-mac / lan-ubuntu | clone 76e7528 | **双红**：E0382 + setsid unsafe 被 -D warnings 拒 → 全部 unix-only |

## Round 0→2 修复链（peer 发现 → 本机修 → 推送）

| # | 发现（发现台） | 根因 | 修复 commit |
| --- | --- | --- | --- |
| 1 | E0382 move（mac+ubuntu） | config_path 在 218 行 if-let 被 move；首轮只修了下游借用（282），根因没动——Windows 编译不到 282 所以看不见 | f60c413（下游）→ **f9e78a5（根因借用化）** |
| 2 | setsid unsafe 块 -D warnings 拒（mac+ubuntu） | workspace unsafe_code=warn；unix 分支的 FFI 边界没给 scoped allow | f9e78a5（allow + SAFETY 注释，与 Windows 探活同待遇） |
| 3 | **with-node.sh CRLF → dash 解析炸 → hooks 全失效**（仅 ubuntu） | Windows 检出 CRLF，dash 不认 | ubuntu peer 本地热修 + **.gitattributes 钉 LF**（\*.sh/\*.mjs/justfile）防复发 |
| 4 | `--cwd ~/...` spawn_failed / 短名 not_found / 旧 vulnforge peer 占名 / NEXEST missing / 分号吞退出码假绿 | herdr 操作形态 | 全部固化进 verify skill 坑表 |

## Round 2 终局

| 台 | git | 门 | 测试 | SKIP |
| --- | --- | --- | --- | --- |
| Windows | f9e78a5 | 全绿 exit 0 | 45/45 | 无 |
| lan-mac（t1p） | f9e78a5 | fmt/clippy/nextest/doc/system-lint 全 0 | 45/45 | 无 |
| lan-ubuntu（t2c） | f9e78a5 | 同上全 0（daemon e2e 4 项真实执行） | 45/45 | 无 |

**三台全绿收口。**

## 结论

1. **三平台轮是唯一能暴露 unix-only 缺陷的层**：三个真 bug（E0382 根因、unsafe allow、CRLF）没有一个在 Windows 本地可见——cfg(windows)/cfg(unix) 分支互为盲区
2. **常驻 peer 会话形态成立**：一轮一任务、全地址派发、Monitor 状态等待；两 peer 会话保持驻留（lan-mac/t1p、lan-ubuntu/t2c）待后续轮
3. CRLF 是跨平台仓的隐性炸弹（hooks 静默失效比报错更危险）；`.gitattributes` 从此是标准资产
4. 失败分类法首轮实战：三个发现全部正确归类"平台真实差异"（非环境/陈旧）
