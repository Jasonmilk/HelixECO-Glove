# Helix ECO Glove

> Helix 生态的原生操作系统/设备适配层。所有手套实现标准 `EcoGlove` trait，可被 Helix-Tentacle 以平台感知方式加载。

**当前状态**：P1 进行中 — EcoGlove trait + macOS Glove 最小版本（26 测试全绿）

---

## 什么是 Helix ECO Glove？

Helix ECO Glove 是 Helix 生态的**原生适配层**。它提供标准化的、平台特定的工具（文件系统、进程、脚本、传感器等），可被 Helix-Tentacle 加载和执行。

每个"手套"是特定平台（macOS、Linux、鸿蒙、Android、ROS2 机器人等）的原生适配器，实现统一的 `EcoGlove` trait。这使得 Tentacle 只需实现一次插件加载器，即可跨所有平台工作。

### 设计哲学

| 原则 | 体现 |
|---|---|
| **极致解耦** | 手套只负责执行；Tentacle 负责调度；Anaphase 负责编排；Mind 负责决策 |
| **按需加载** | 只加载当前平台支持的手套（编译时 `cfg` + 运行时白名单） |
| **按需驱动** | 事件驱动，无轮询。未调用时零能耗 |
| **极致复用** | 所有手套共享 `EcoGlove` trait；Tentacle 只需实现一次加载器 |
| **物理事实优先** | 平台、风险等级、能力域都是物理事实，不可伪造 |
| **确定性优先** | 相同输入产生相同输出；无隐藏状态或副作用（除非显式声明） |

---

## 项目结构

```
HelixECO-Glove/
├── Cargo.toml              # Workspace 根配置
├── README.md               # 英文版
├── README.zh-cn.md         # 中文版（本文件）
├── core/                   # 标准接口定义
│   ├── Cargo.toml
│   └── src/lib.rs          # EcoGlove trait + 所有标准类型
├── gloves/                 # 平台特定手套实现
│   └── macos/              # macOS 原生手套
│       ├── Cargo.toml
│       └── src/lib.rs
├── tests/                  # 集成测试
└── docs/
    ├── DNA.md              # 宪法：6 条不可变原则
    ├── RNA.md              # 加载协议：AI 如何读取本仓库
    ├── PLAN.md             # 导航：当前阶段 + 下一阶段预览
    ├── GROWTH.md           # 生长记录：最近 3 条健康快照
    └── decisions/          # 架构决策记录（ADR）
```

---

## EcoGlove trait 概览

`EcoGlove` trait 是所有手套的宪法。每个手套必须实现：

### 元信息（静态，物理事实）

| 方法 | 说明 |
|---|---|
| `platform()` | 平台标识（macOS、Linux、Harmony、Robot 等） |
| `host_os()` | 支持的宿主 OS 列表（用于 Tentacle 白名单过滤） |
| `version()` | 语义化版本 |
| `name()` | 手套名称（如 "macos-glove"） |
| `description()` | 手套描述 |

### 工具定义

| 方法 | 说明 |
|---|---|
| `tools()` | `ToolDefinition` 列表 — 名称、描述、能力域、风险等级、参数、返回值描述 |
| `capabilities()` | 能力域列表（默认从工具中推导） |
| `get_tool(name)` | 按名称查找工具 |

### 执行

| 方法 | 说明 |
|---|---|
| `execute(tool, params, dry_run)` | 异步执行工具。返回 `ToolResult` |
| `validate_parameters(tool, params)` | 执行前验证参数（提供默认实现） |

### 生命周期

| 方法 | 说明 |
|---|---|
| `initialize()` | 初始化手套（加载配置、建立连接） |
| `shutdown()` | 清理资源 |

### 平台门控

| 方法 | 说明 |
|---|---|
| `is_supported_on_current_platform()` | 检查手套是否在当前平台受支持 |

---

## 工具命名规范

所有工具使用**点分命名空间**：

```
<platform>.<domain>.<action>
```

示例：

| 工具名 | 平台 | 能力域 | 动作 | 风险 |
|---|---|---|---|---|
| `macos.fs.read_file` | macOS | filesystem | read_file | LOW |
| `macos.fs.write_file` | macOS | filesystem | write_file | MEDIUM |
| `macos.fs.list_directory` | macOS | filesystem | list_directory | LOW |
| `macos.process.execute` | macOS | process | execute | CRITICAL |
| `macos.process.list` | macOS | process | list | LOW |
| `macos.script.applescript` | macOS | script | applescript | CRITICAL |

---

## 风险等级

与 CI-144 PFP（物理特征协议）对齐：

| 等级 | 含义 | 示例 | 需要确认 |
|---|---|---|---|
| **LOW** | 只读操作 | read_file、list_directory、list_processes | 否 |
| **MEDIUM** | 写操作 | write_file、create_directory | 否 |
| **CRITICAL** | 可能影响系统稳定性 | execute_command、delete_file | 是 |
| **CATASTROPHIC** | 可能造成不可逆损害 | system_update、hardware_control | 是（多重确认） |

---

## macOS Glove（当前实现）

6 个原生 macOS 工具：

| 工具 | 说明 | 风险 | dry_run |
|---|---|---|---|
| `macos.fs.read_file` | 读取文件 | LOW | ✅ |
| `macos.fs.write_file` | 写入文件 | MEDIUM | ✅ |
| `macos.fs.list_directory` | 列出目录 | LOW | ✅ |
| `macos.process.execute` | 执行 shell 命令 | CRITICAL | ❌ |
| `macos.process.list` | 列出运行进程 | LOW | ✅ |
| `macos.script.applescript` | 执行 AppleScript | CRITICAL | ❌ |

### 快速示例

```rust
use helix_eco_glove_core::*;
use helix_eco_glove_macos::MacOSGlove;

#[tokio::main]
async fn main() -> Result<(), GloveError> {
    let glove = MacOSGlove::new();
    glove.initialize()?;

    // 读取文件
    let result = glove
        .execute(
            "macos.fs.read_file",
            serde_json::json!({"path": "/etc/hosts"}),
            false,
        )
        .await?;

    println!("成功: {}", result.success);
    println!("内容: {}", result.data.unwrap()["content"]);

    glove.shutdown()?;
    Ok(())
}
```

---

## 平台隔离（双层门控）

### 第一层：编译时排除

```rust
// 在 Tentacle 的 Cargo.toml 或代码中：
#[cfg(target_os = "macos")]
use helix_eco_glove_macos::MacOSGlove;

#[cfg(target_os = "linux")]
use helix_eco_glove_linux::LinuxGlove;
```

### 第二层：运行时白名单

```rust
// Tentacle 的插件加载器：
let current_os = std::env::consts::OS;
for glove in discovered_gloves {
    if glove.is_supported_on_current_platform() {
        registry.register(glove);
    } else {
        tracing::info!("跳过手套 {}: 仅支持 {:?}", glove.name(), glove.host_os());
    }
}
```

---

## 测试

```bash
# 构建整个 workspace
cargo build

# 运行所有测试
cargo test

# 只测试 core
cargo test -p helix-eco-glove-core

# 只测试 macOS 手套
cargo test -p helix-eco-glove-macos

# 检查代码格式
cargo fmt --check

# 运行 clippy 静态检查
cargo clippy --all-targets -- -D warnings
```

**当前测试结果**：26 个测试全绿（12 core 单元 + 13 macOS 单元 + 1 文档测试）

---

## Helix 生态

| 项目 | 角色 | 状态 |
|---|---|---|
| [Helix-Mind](https://github.com/Jasonmilk/Helix-Mind) | 大脑（记忆/认知） | ✅ 核心完成 |
| [Anaphase-Helix](https://github.com/Jasonmilk/Anaphase-Helix) | 躯干（编排/执行） | ✅ 待决策 |
| [Helix-Tentacle](https://github.com/Jasonmilk/Helix-Tentacle) | 手（工具执行） | ✅ 完成 |
| [Tuck](https://github.com/Jasonmilk/Tuck) | 免疫系统（安全闸门） | ✅ 完成 |
| [Cellrix](https://github.com/Jasonmilk/Cellrix) | 皮肤（UI/显示） | ✅ 完成 |
| [BIND-19](https://github.com/CommonIntents/BIND-19) | 神经系统（CI-144 协议） | ✅ 完成 |
| [Helix-MCP-Learner](https://github.com/Jasonmilk/Helix-MCP-Learner) | 翻译官（MCP → CI-144） | ✅ P2 完成 |
| **HelixECO-Glove** | **原生适配器（OS/设备手套）** | **🚧 P1 进行中** |

---

## 治理

本项目遵循 **phyt-DNA 方法论 v1.0**。

| 文档 | 用途 |
|---|---|
| [docs/DNA.md](docs/DNA.md) | 宪法：6 条不可变原则 |
| [docs/RNA.md](docs/RNA.md) | 加载协议：AI 如何读取本仓库 |
| [docs/PLAN.md](docs/PLAN.md) | 导航：当前阶段 + 下一阶段预览 |
| [docs/GROWTH.md](docs/GROWTH.md) | 生长记录：最近 3 条健康快照 |
| [docs/decisions/](docs/decisions/) | 架构决策记录（ADR） |

---

## 路线图

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P1** | **项目初始化 + EcoGlove trait + macOS Glove 最小版本** | **🚧 进行中** |
| P2 | Tentacle 集成 + 平台感知加载器 | ⏳ 预览 |
| P3 | MCP-Learner 标记 deprecated + 审查体系 L1（静态检查） | ⏳ 预览 |
| P4 | 审查体系 L2（dry_run 沙箱）+ 规则自进化 | ⏳ 预览 |
| P5 | 新平台手套（Linux/鸿蒙/机器人）+ 审查体系 L3 | ⏳ 预览 |

---

## 许可证

MIT OR Apache-2.0

---

**作者**：Jasonmilk / CommonIntents
**创建日期**：2026-08-31
