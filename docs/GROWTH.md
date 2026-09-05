# Helix ECO Glove — 生长记录

> **版本**：v1.2
> **日期**：2026-08-31
> **所属方法论**：phyt-DNA 方法论 v1.0
> **规则**：仅保留最近 3 条记录，超则归档至 `docs/archive/growth/`（已版本化，永不删除）

---

## [2026-08-31] P3 完成 — MCP-Learner deprecated 标记 + 审查体系 L1

### 触发条件
P2 完成后，需要清理 MCP-Learner 中的旧 glove 代码，并建立审查体系 L1（静态检查），确保 MCP-Learner 学习生成的工具 Manifest 格式正确、风险等级合理、参数 Schema 完整。

### 变更性质
- **MCP-Learner 清理**：`src/glove/` 模块标记为 deprecated，说明已迁移至 HelixECO-Glove，计划 v0.3.0 移除
- **审查体系 L1**：在 `core/src/reviewer.rs` 中实现 `StaticReviewer`，提供 9 条审查规则（R001-R009）
- **审查结果类型**：`Severity`（Info/Warning/Error）、`ReviewFinding`、`ReviewReport`
- **工具 Manifest 最小子集**：`ToolManifest`（不依赖 tentacle-core，保持 core 纯净）

### 关键数据
- 审查规则：9 条（R001 名称非空 / R002 SemVer 版本 / R003 描述 / R004 参数 Schema / R005 风险等级 / R006 平台标识 / R007 host_os / R008 高风险工具详情 / R009 点分命名空间）
- reviewer 测试：10 个全绿
- 总测试数：44 个（22 core + 13 macos + 8 adapter + 1 doc）

### 架构决策
- **审查器放在 core crate**：保持纯净，不依赖 tentacle-core，可被任何项目使用
- **三级严重级别**：Info（不阻塞）/ Warning（建议修复）/ Error（必须修复）
- **审查体系三层架构**：L1 静态检查（本阶段）→ L2 dry_run 沙箱预执行 → L3 人类审查
- **可演化设计**：审查规则版本化（RULES_VERSION），支持 L2 反馈自动升级 L1 规则

### 下一步
P4 — 审查体系 L2（dry_run 沙箱预执行）+ 规则自进化机制

---

## [2026-08-31] P2 完成 — Tentacle 集成 + 平台感知加载器

### 触发条件
P1 完成后，需要将 HelixECO-Glove 集成到 Helix-Tentacle，使 Tentacle 可以统一加载和执行所有平台手套。同时需要实现平台感知的插件加载器，防止"在 macOS 上用鸿蒙插件"。

### 变更性质
- **新增适配器 crate**：`adapters/tentacle/`，将 EcoGlove 实现转换为 Tentacle 的 Manifest + Tool
- **Tentacle Manifest 扩展**：新增 `PlatformSupport` 结构体（platform + host_os），与 CI-144 PFP 和 EcoGlove trait 对齐
- **ToolRegistry 平台过滤**：新增 6 个平台感知方法（index_for_current_platform / index_for_os / is_supported_on_current_platform / supported_tool_names / unsupported_tool_names）
- **GloveAdapter**：提供 register_supported（只注册当前平台支持的工具）、manifests()、tools()、manifest_indices() 等便捷方法
- **工具命名规范**：点分命名空间 `<platform>.<domain>.<action>`（如 macos.fs.read_file）

### 关键数据
- 适配器测试：8 个单元测试 + 1 个文档测试全绿
- 总测试数：34 个（12 core + 13 macos + 8 adapter + 1 doc）
- Tentacle-core 测试：52 个全绿（含新增平台过滤方法）
- 新增代码：约 500 行（适配器 lib.rs）

### 架构决策
- **适配器放在 HelixECO-Glove 仓库**：极致解耦，Tentacle 不需要知道 EcoGlove 的存在
- **平台过滤双层保险**：编译时 cfg(target_os) 物理排除 + 运行时 host_os() 白名单过滤
- **异步转同步**：EcoGlove 的 execute 是 async，适配器用 tokio::runtime::Handle::block_on 转换为 Tentacle 的同步 Tool trait

### 下一步
P3 — MCP-Learner 标记 `src/glove/` 为 deprecated + 审查体系 L1（静态检查）

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

## [2026-09-06] P4-T1 完成 — L1 与 MCP-Learner 管道集成 + 状态迁移自动化

### 触发条件
P4-T1 完成：L1 静态审查接入 MCP-Learner 管道，raw/staging/stable/rejected 状态迁移自动化。

### 变更性质
- **post_learn 管道集成**：MCP-Learner 新增 `post_learn` 模块（ReviewPipeline + ReviewPipelineConfig + ToolState + ToolReviewResult + BatchReviewResult）
- **状态迁移自动化**：Error → rejected/，Warning → staging/，Info/无问题 → stable/
- **审查报告**：`{server}_review_report.json` 生成
- **集成点**：`mcp-learner learn` 命令学习完成后自动审查并输出状态统计

### 关键成果
- **测试**：8 个 post_learn 测试全绿（MCP-Learner 总 50 个全绿——注：此后 1 个测试回归为 failed，ECOSYSTEM 已记录待修）
- **生态闭环**：MCP-Learner 学习 → Glove L1 审查 → stable/ → Tentacle 加载执行

### 提交
- `ea923fe`（P3：deprecated marker + Reviewer L1）+ `744cf4e`/`cfeb594`（P4-T1 文档）

### 状态
✅ P4-T1 完成；P4-T2（dry_run 沙箱预执行）待启动

---

**最后更新**：2026-09-06
