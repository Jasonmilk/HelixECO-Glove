# Helix ECO Glove — 生长记录

> **版本**：v1.0
> **日期**：2026-08-31
> **所属方法论**：phyt-DNA 方法论 v1.0
> **规则**：仅保留最近 3 条记录，超则归档至 `docs/archive/growth/`（已版本化，永不删除）

---

## [2026-08-31] 项目初始化 — EcoGlove trait + macOS Glove 最小版本

### 触发条件
Helix 生态对齐讨论后，决定将原生 OS 适配从 MCP-Learner 中剥离，独立为 HelixECO-Glove 项目。采用"先合后拆"的 Monorepo 结构，所有平台手套在一个 workspace 中，成熟后再拆分为独立仓库。

### 变更性质
- **项目初始化**：创建 HelixECO-Glove 独立仓库，Cargo workspace 结构，MIT/Apache 2.0 双协议
- **方法论骨架**：DNA.md（6 条不可变原则）、RNA.md（三层加载协议 + 7 条 AI 协作铁律）、PLAN.md（P1 导航牌）、GROWTH.md（本记录）
- **EcoGlove trait 定义**：Platform（8 种平台）、RiskLevel（4 级）、CapabilityDomain（10 种能力域）、ToolDefinition、ToolParameter、ToolResult、GloveError
- **macOS Glove 最小版本**：6 个工具（macos.fs.read_file / macos.fs.write_file / macos.fs.list_directory / macos.process.execute / macos.process.list / macos.script.applescript）
- **平台隔离机制**：编译时 cfg(target_os) + 运行时 host_os() 白名单，双层保险
- **dry_run 支持**：只读工具支持无副作用预执行

### 关键成果
- **测试**：26 个全绿（12 core 单元 + 13 macos 单元 + 1 文档测试）
- **工具数**：6 个（3 文件系统 + 2 进程管理 + 1 脚本执行）
- **平台数**：1 个（macOS），预留 Linux/Windows/Harmony/Android/Robot 等扩展位
- **EcoGlove trait 方法数**：12 个（platform/host_os/version/name/description/tools/capabilities/is_supported_on_current_platform/get_tool/validate_parameters/initialize/shutdown/execute）

### 兼容性
- 新项目，零历史代码
- 与 Helix 生态通过 EcoGlove trait 解耦，Tentacle 只需实现一次加载器
- 与 CI-144 协议家族对齐：Platform 对应 PFP，RiskLevel 对应 PFP Risk-Level，ToolDefinition 对应 INTENT-7 + CAPABILITY-13

### 验收
- Cargo workspace 编译通过
- 26 个测试全绿
- EcoGlove trait 文档完整，文档测试通过
- macOS Glove 6 个工具全部实现，每个工具有参数验证
- 方法论文档齐全（DNA/RNA/PLAN/GROWTH/ADR）

### 状态
🧬 P1 T1-T4 已完成，T5（README + 提交推送）待启动

---

## [2026-08-31] 生态对齐讨论 — 决定独立 HelixECO-Glove 项目

### 触发条件
P2 完成后，发现 MCP-Learner 中包含 macOS Glove 代码，违背了"单一职责"原则。经过多轮讨论，决定将原生 OS 适配从 MCP-Learner 中剥离，独立为 HelixECO-Glove 项目。

### 关键决策
1. **项目结构**：Monorepo（先合后拆），所有手套在一个 workspace 中
2. **平台隔离**：编译时 cfg + 运行时白名单，双层保险
3. **工具命名**：点分命名空间 `<platform>.<domain>.<action>`
4. **风险评级**：与 CI-144 PFP 对齐
5. **审查体系**：分三阶段落地（P3 L1 静态检查 → P4 L2 dry_run → P5 L3 人类审查）
6. **MCP-Learner 清理**：P3 标记 deprecated，P4 彻底删除 src/glove/

### 状态
🧬 决策已确认，项目初始化中

---

**最后更新**：2026-08-31
