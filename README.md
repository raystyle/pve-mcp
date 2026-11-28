# pve-mcp

> Proxmox VE 虚拟化平台 MCP server：单节点 PVE 集群查询与受控管理的本地 daemon 服务。17 个 MCP 工具（集群/虚拟机/快照/备份/防火墙/QGA）、破坏性操作 confirm 护栏、pvebot token 认证、自部署 install/update。三平台（Windows/Linux/macOS）CI + tag 发布。

## 工具面（17）

| 域 | 工具 |
| --- | --- |
| 查询（只读） | `cluster_status` `vm_list` `vm_status` `vm_metrics` `vm_agent`（QGA osinfo/interfaces）`storage_list` `task_status` |
| 受控操作 | `vm_power` `vm_create`（模板克隆）`vm_delete` `vm_config` `snapshot_manage` `backup_manage`（vzdump）`firewall_manage`（guest 级规则）`lxc_exec` `vm_agent_exec` |

护栏：写操作 dry-run 默认 → `confirm=true` 执行；`protected_vmids` 无条件保护名单；`access_tier = read_only` 只降级查询面；MCP annotations 标注只读/破坏性。

## 安装

### 方式一：下载发布包（推荐）

从 [Releases](https://github.com/raystyle/pve-mcp/releases) 取对应平台包（Windows x64 / Linux x64 / macOS arm64），解压后：

```bash
# Windows（自动加入用户 PATH，无需管理员）
pve-mcp.exe install

# Linux / macOS（装到 ~/.local/bin）
./pve-mcp install
```

install 做三件事：复制 `pve-mcp`（MCP server）+ `pve-cli`（命令行）到用户目录、加 PATH（Windows 写注册表并广播；Unix 提示 export 行）、daemon 运行中则无伤停-换-启。升级用 `update`（同 install，覆盖式）。

### 方式二：源码构建

```bash
git clone https://github.com/raystyle/pve-mcp && cd pve-mcp
cargo build --release
./target/release/pve-mcp install     # 或直接用 target/release/ 下的二进制
```

要求：Rust stable（1.98+）。测试需 `cargo-nextest`；完整门 `just ci`（fmt → clippy -D warnings → nextest → doc tests → system-lint）。

## 配置

凭据只放 gitignored 的 `pve-mcp.toml`（模板 `pve-mcp.toml.example`），查找顺序：`--config` > `PVE_MCP_CONFIG` 环境变量 > 应用数据目录（`%APPDATA%/pve-mcp`，Unix `~/.config/pve-mcp`）> exe 同目录 > 工作目录。

### 1. 建 PVE API token（最小权限）

在 PVE 宿主上执行（只读面）：

```bash
pveum user add pvebot-ro@pve
pveum acl modify / --users pvebot-ro@pve --roles PVEAuditor
pveum user token add pvebot-ro@pve mcp --privsep 1 --output-format json
# 输出的 value 即 token secret（只显示一次）
```

### 2. 写配置

```toml
[pve]
host = "192.168.88.88"     # PVE 地址
port = 8006
verify_ssl = false          # 自签集群
token_id = "pvebot-ro@pve!mcp"
token_value = "<上一步的 value>"

[security]
protected_vmids = "106"     # 这些 vmid 任何工具不得改动
```

密码 ticket 备用通道、SSH 通道（`lxc_exec` 用）见 example 注释。

### 3. 接入 Claude Code（项目级注册）

项目根 `.mcp.json`（gitignore，本机路径各配）：

```json
{
  "mcpServers": {
    "pve-mcp": {
      "type": "stdio",
      "command": "pve-mcp",
      "args": ["stdio"]
    }
  }
}
```

Windows 可用变量形 `"command": "${LOCALAPPDATA}/pve-mcp/pve-mcp.exe"`。重启会话即加载；daemon 首次调用自动拉起（无控制台窗口）。

### 4. 验证

```bash
pve-cli cluster status        # 或在 Claude Code 会话里直接问"集群概况"
```

## CLI（与 MCP 工具 1:1）

```bash
pve-cli cluster status | list --ty lxc
pve-cli vm status 102 | start 102 | shutdown 102 --confirm
pve-cli vm create --name test-1 --template 100 --confirm
pve-cli vm delete 102 --confirm
pve-cli daemon status | stop
pve-cli <verb> --json         # 机器可读信封
```

## 架构

```
MCP 客户端 ─ stdio 桥 ┐
pve-cli 动词 ─────────┤→ daemon（单实例，typestate 生命周期）
                     │    ├─ 护栏管道：tier → vmid → protected → confirm → dry-run
                     └─ admin plane（127.0.0.1 TCP）
                          └─ PveClient → PVE /api2/json（token/ticket 双认证）
```

- 品牌域类型（Vmid/NodeName/SnapshotName/Upid newtype，构造即校验）
- daemon 冷启动：探测-拉起-等就绪；锁文件单实例互斥 + 僵尸接管
- 三消费形态：MCP stdio / CLI 动词 / daemon 管理面

## 开发

```bash
just ci          # 完整门：fmt → clippy -D warnings → nextest → doc tests → system-lint
cargo test -p pve-core
```

67 测试（纯函数 + daemon e2e + mock-PVE TLS 集成套件）；三平台 CI（windows/ubuntu/macos-latest）；`v*` tag 触发发布。

## License

MIT
