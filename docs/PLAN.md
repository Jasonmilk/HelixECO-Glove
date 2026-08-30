# Helix ECO Glove — 开发导航牌（PLAN）

> **版本**：v1.2（P2 完成，2026-08-31）
> **状态**：✅ P2 完成 — Tentacle 集成 + 平台感知加载器
> **分支**：main
> **所属方法论**：phyt-DNA 方法论 v1.0
> **规则**：本文件只含当前阶段 + 下一阶段预览 + 阶段总览地图。完成阶段 → GROWTH.md。总行数 ≤150，超出触发历史迁移。

---

## 1. 当前阶段：P3 预览 — MCP-Learner 清理 + 审查体系 L1

> **状态**：⏳ 待启动。
> **目标**：在 MCP-Learner 中标记 `src/glove/` 为 deprecated，说明已迁移到 HelixECO-Glove。实现审查体系 L1（静态检查）。
> **前置依赖**：P2 完成（Tentacle 集成 + 平台感知加载器）。

### 1.1 任务拆分（P1 已完成）

| 任务 | 内容 | 状态 |
|---|---|---|
| T1 | 项目初始化：Cargo workspace + 目录结构 + 方法论骨架（DNA/RNA/PLAN/GROWTH/ADR） | ✅ 完成 |
| T2 | EcoGlove trait 定义：Platform/RiskLevel/CapabilityDomain/ToolDefinition/ToolResult/GloveError | ✅ 完成 |
| T3 | macOS Glove 最小版本：6 个工具（文件读写/目录列表/命令执行/进程列表/AppleScript） | ✅ 完成 |
| T4 | 测试：单元测试 + 文档测试，覆盖率 ≥ 80% | ✅ 完成（26 测试全绿） |
| T5 | 文档：README 中英文版 + 提交 + 推送 GitHub | ✅ 完成 |

### 1.2 代码真相源

- **EcoGlove trait（T2 ✅）**：`core/src/lib.rs` — 标准接口定义，所有手套必须实现
- **macOS Glove（T3 ✅）**：`gloves/macos/src/lib.rs` — macOS 原生系统工具适配
- **工具命名规范**：点分命名空间 `<platform>.<domain>.<action>`（如 `macos.fs.read_file`）

### 1.3 关键决策点（已确认）

| # | 决策点 | 最终方案 |
|---|---|---|
| D1 | 项目结构 | Monorepo（先合后拆），所有手套在一个 workspace 中 |
| D2 | 平台隔离 | 编译时 cfg + 运行时 host_os() 白名单，双层保险 |
| D3 | 工具命名 | 点分命名空间：`<platform>.<domain>.<action>` |
| D4 | 风险评级 | 与 CI-144 PFP 对齐：LOW/MEDIUM/CRITICAL/CATASTROPHIC |
| D5 | dry_run | 只读工具必须支持，有副作用工具可选 |

### 1.4 验收标准

- T1：Cargo workspace 编译通过，方法论文档齐全
- T2：EcoGlove trait 定义完整，文档测试通过
- T3：macOS Glove 实现 6 个工具，每个工具有参数验证
- T4：26 个测试全绿，覆盖率 ≥ 80%
- T5：README 中英文版完善，代码已推送 GitHub

---

## 2. 下一阶段预览：P2 — Tentacle 集成 + 平台感知加载器

> **状态**：⏳ 预览中。
> **目标**：将 HelixECO-Glove 集成到 Helix-Tentacle，实现平台感知的插件加载器。
> **前置依赖**：P1 完成。

### 2.1 任务预览

| 任务 | 内容 | 状态 |
|---|---|---|
| T1 | 审查 Tentacle 当前架构，确认插件加载机制 | ⏳ 预览 |
| T2 | 实现 Tentacle 平台感知加载器（编译时 cfg + 运行时白名单） | ⏳ 预览 |
| T3 | CLI 平台标签显示（`tentacle-cli tools list` 显示平台标签） | ⏳ 预览 |
| T4 | 集成验证：Tentacle 加载 macOS Glove + 端到端测试 | ⏳ 预览 |
| T5 | 文档 + 提交 | ⏳ 预览 |

---

## 3. 阶段总览地图

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P1** | **项目初始化 + EcoGlove trait + macOS Glove 最小版本** | **✅ 完成** |
| **P2** | **Tentacle 集成 + 平台感知加载器** | **✅ 完成** |
| P3 | MCP-Learner 标记 deprecated + 审查体系 L1（静态检查） | ⏳ 预览 |
| P4 | 审查体系 L2（dry_run 沙箱预执行）+ 规则自进化 | ⏳ 预览 |
| P5 | 新平台手套（Linux/鸿蒙/机器人）+ 审查体系 L3（人类审查接口） | ⏳ 预览 |

---

## 4. 当前测试战绩

| 模块 | 测试数 | 状态 |
|---|---|---|
| core（EcoGlove trait） | 12 单元 + 1 文档 | ✅ 全绿 |
| gloves/macos（macOS Glove） | 13 单元 | ✅ 全绿 |
| **总计** | **26** | **✅ 全绿** |

---

**最后更新**：2026-08-31
