# Helix ECO Glove — DNA（宪法）
> © 2026 Jason Milk · Apache 2.0

> **版本**：v1.0
> **日期**：2026-08-31
> **性质**：不可变宪法，任何实现与扩展不得逾越此 6 条原则。
> **所属方法论**：phyt-DNA 方法论 v1.0

---

## 六条不可变原则

### 原则 I：极致解耦（Maximum Decoupling）

手套只负责"执行"，不负责"决策"。Tentacle 负责调度，Anaphase 负责编排，Helix-Mind 负责决策。手套是纯粹的执行层，不包含任何业务逻辑或决策逻辑。

**体现**：
- EcoGlove trait 只定义元信息 + 工具定义 + 执行，不定义决策
- 手套不依赖 Tentacle/Anaphase/Mind，只依赖 core crate
- 每个手套是独立的 workspace 成员，可单独编译和测试

### 原则 II：按需加载（On-Demand Loading）

只加载当前平台需要的手套，不加载不支持的平台。编译时用 `cfg(target_os)` 排除，运行时用 `host_os()` 白名单过滤。

**体现**：
- 每个手套声明 `host_os()`，Tentacle 加载时做白名单过滤
- 编译时可用 `cfg(target_os = "macos")` 物理排除不支持的手套
- 工具级别的按需加载：手套可以提供多个工具，但只在调用时执行

### 原则 III：按需驱动（On-Demand Execution）

事件驱动，无轮询。手套不主动检查任何状态，只在被调用时执行。无调用时零能耗。

**体现**：
- EcoGlove trait 没有 `poll()` 或 `tick()` 方法
- 所有工具都是被动执行，由 Tentacle 调用触发
- 手套不持有后台线程或定时器

### 原则 IV：极致复用（Maximum Reuse）

所有手套共享同一套标准接口（EcoGlove trait），Tentacle 只需实现一次加载器。工具定义、参数 schema、风险等级、执行结果都是标准化的，可跨平台复用。

**体现**：
- EcoGlove trait 是所有手套的"宪法"，定义了标准接口
- ToolDefinition、ToolParameter、RiskLevel、ToolResult 都是标准化类型
- 手套之间可以共享工具定义（如文件系统工具在 macOS/Linux 上有相同的接口）
- 审查器、CLI 包装器等组件只需实现一次，适用于所有手套

### 原则 V：物理事实优先（Physical Facts First）

平台标识、宿主 OS、能力声明都是物理事实，不可伪造。风险等级基于工具的实际物理影响（删除=CRITICAL，读取=LOW），而非 AI 的语义判断。

**体现**：
- `platform()` 返回编译时确定的平台标识，不可运行时修改
- `host_os()` 返回静态字符串列表，不可动态改变
- 风险等级在工具定义时固定，执行时不可降级
- 参数验证基于物理类型（string/number/boolean），而非语义推断

### 原则 VI：确定性优先（Determinism First）

工具定义、参数 schema、风险等级都是固定的，执行结果可预测。相同输入产生相同输出，无隐藏状态或副作用（除非工具本身就是有副作用的，如写文件）。

**体现**：
- 工具定义是静态的，不随运行时状态变化
- 参数验证是确定性的，相同参数总是通过或失败
- 只读工具（read_file/list_directory/list_processes）是纯函数，无副作用
- 有副作用的工具（write_file/execute_command）显式声明风险等级，需要人工确认
- `dry_run` 模式提供无副作用的预执行，用于验证

---

## 与 CI-144 协议家族的对齐

| CI-144 概念 | EcoGlove 对应 |
|---|---|
| PFP（物理特征协议） | Platform + RiskLevel + CapabilityDomain |
| INTENT-7（语义层） | ToolDefinition.name（点分命名空间） |
| CAPABILITY-13（能力层） | ToolParameter + parameters_json_schema() |
| BIND-19（传输层） | Tentacle 插件加载机制 |
| Tuck（免疫层） | 风险等级 + 人工确认 + dry_run |

---

## 宪法守护

本文件是 Helix ECO Glove 的最高法律。任何新增手套、修改接口、扩展功能，都必须先通过此 6 条原则的审查。违反原则的 PR 将被拒绝，无论其功能多么有用。

**修改流程**：
1. 提交 ADR（架构决策记录），说明为何需要修改宪法
2. 经过至少 3 个核心贡献者审查
3. 全票通过后方可修改
4. 修改后版本号递增，旧版本永久保留

---

**签署**：Jasonmilk / CommonIntents
**日期**：2026-08-31
