# Helix ECO Glove — RNA（加载协议）
> © 2026 Jason Milk · Apache 2.0

> **版本**：v1.0
> **日期**：2026-08-31
> **性质**：AI 协作加载协议，定义 AI 如何读取、理解和操作本仓库。
> **所属方法论**：phyt-DNA 方法论 v1.0

---

## 三层加载协议

### 第一层：宪法层（DNA.md）—— 不可变原则

AI 在执行任何操作前，必须先读取 `docs/DNA.md`，理解 6 条不可变原则。任何操作不得违反这些原则。

**必须遵守**：
- 极致解耦：手套只负责执行，不负责决策
- 按需加载：只加载当前平台需要的手套
- 按需驱动：事件驱动，无轮询
- 极致复用：所有手套共享 EcoGlove trait
- 物理事实优先：平台、风险等级是物理事实，不可伪造
- 确定性优先：相同输入产生相同输出

### 第二层：导航层（PLAN.md）—— 当前阶段与任务

AI 读取 `docs/PLAN.md`，了解当前开发阶段、任务拆分、关键决策点和验收标准。

**必须遵守**：
- 只执行当前阶段的任务，不超前开发
- 任务完成后更新 PLAN.md 状态
- 关键决策点需要用户确认后才能继续
- 验收标准全部通过后才能标记任务完成

### 第三层：生长层（GROWTH.md + ADR/）—— 历史与决策

AI 读取 `docs/GROWTH.md` 了解项目生长历史，读取 `docs/decisions/` 了解架构决策记录。

**必须遵守**：
- 重大架构变更必须创建 ADR
- 阶段完成后必须在 GROWTH.md 中记录
- 参考历史决策，避免重复讨论已解决的问题

---

## 7 条 AI 协作铁律

### 铁律 1：决策拦截

任何架构/接口变更时，必须提示创建 `docs/decisions/ADR-<编号>-<标题>.md`。

- 编号为 4 位数字，按顺序递增（0001, 0002, ...）
- 引用时使用 `ADR-<编号>`（如 ADR-0001）
- 文件名与引用名必须一致

### 铁律 2：测试先行

任何新功能必须先写测试，再写实现。测试覆盖率不得低于 80%。

- 单元测试：每个工具至少 3 个测试（正常、异常、边界）
- 集成测试：端到端验证
- 文档测试：trait 和重要类型必须有文档示例

### 铁律 3：平台门控

新增手套时，必须明确声明 `platform()` 和 `host_os()`。Tentacle 加载时做白名单过滤，绝不允许在不支持的平台上注册。

### 铁律 4：风险评级

每个工具必须明确声明 `risk_level`。

- LOW：只读操作（读取文件、列出目录、查看进程）
- MEDIUM：写操作（创建文件、更新配置）
- CRITICAL：可能影响系统稳定性（执行命令、删除文件）—— 需要人工确认
- CATASTROPHIC：可能造成不可逆损害（系统更新、硬件操作）—— 需要多重确认

### 铁律 5：dry_run 支持

只读工具必须支持 `dry_run` 模式。有副作用的工具可以不支持 dry_run，但必须在工具定义中显式声明 `supports_dry_run = false`。

### 铁律 6：参数验证

所有工具执行前必须验证参数。必填参数缺失、参数类型错误时，必须返回明确的错误信息，不得 panic 或静默忽略。

### 铁律 7：文档同步

代码变更必须同步更新文档。README、PLAN、GROWTH、ADR 必须与代码保持一致。

---

## 仓库结构导航

```
HelixECO-Glove/
├── Cargo.toml              # Workspace 配置
├── README.md               # 项目说明（英文）
├── README.zh-cn.md         # 项目说明（中文）
├── core/                   # 标准接口定义（EcoGlove trait）
│   ├── Cargo.toml
│   └── src/lib.rs
├── gloves/                 # 各平台手套实现
│   └── macos/              # macOS 手套
│       ├── Cargo.toml
│       └── src/lib.rs
├── tests/                  # 集成测试
└── docs/
    ├── DNA.md              # 宪法（6 条不可变原则）
    ├── RNA.md              # 本文件（加载协议）
    ├── PLAN.md             # 导航（当前阶段 + 任务拆分）
    ├── GROWTH.md           # 生长记录（最近 3 条）
    └── decisions/          # 架构决策记录（ADR）
```

---

## 常用命令

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

---

## 新增手套流程

1. 创建 `gloves/<platform>/` 目录和 Cargo.toml
2. 在 workspace 的 Cargo.toml 中添加新成员
3. 实现 EcoGlove trait
4. 编写单元测试（覆盖率 ≥ 80%）
5. 编写集成测试
6. 更新 README 和 PLAN
7. 创建 ADR（如果是新平台或重大变更）
8. 提交 PR

---

**签署**：Jasonmilk / CommonIntents
**日期**：2026-08-31
