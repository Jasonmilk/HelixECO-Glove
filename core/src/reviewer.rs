//! 审查器（Reviewer）— L1 静态检查
//!
//! 对 MCP-Learner 学习生成的工具 Manifest 进行自动静态审查，
//! 确保格式正确、风险等级合理、参数 Schema 完整、平台兼容性正确。
//!
//! # 审查体系三层架构
//!
//! - **L1（本模块）**：自动机器审查 — 静态检查 Manifest 格式、风险等级、参数 Schema、平台兼容性
//! - **L2**：Helix 主动审查 — 用真实场景测试工具，模拟执行验证（dry_run）
//! - **L3**：人类审查 — L2 也拿不准时提示人类确认
//!
//! # 设计哲学
//!
//! - **极致节能**：L1 在本地运行，不消耗 Token，纯静态分析
//! - **白盒可审计**：所有审查记录、规则版本都明文存储
//! - **可演化**：审查规则随实际执行反馈自动升级（L2 反馈 → L1 规则更新）
//!
//! # R005 的三条通则（`CI-144_跨链生命周期与反馈契约.md`，本次修复的判据）
//!
//! 1. **未知必须显式表示（不得用默认值承载）。** 危险形态是**缺失缺省**：
//!    `rules_path=""`、`upstream_key=""`、以及 `record() -> ()` 这种让
//!    "写失败会拦下动作"**无法表达**的签名。在 R005 这里，它的两个实例是
//!    `risk_level` **为空**（沉默被当成一个合法取值 ⇒ 放行）与
//!    `risk_level` **缺失**（`#[serde(default)]` 把"没有这个字段"变成 `""`）。
//!    ⇒ 处置：**沉默不再能蒙混过关**，它与"声明 unknown"分开判定且不得更轻。
//! 2. **单调性：不确定性上升，不得导致更宽松的处置。**
//!    修复前恰好相反（沉默 `Warning`、声明"我不知道" `Error`）——
//!    **系统惩罚诚实声明、奖励沉默**，而沉默恰好摧毁 `FieldProvenance`。
//!    ⇒ 可接受度序（越小越宽松）：`Accepted < Quarantine < Rejected`，
//!    且 `rank(silence) ≥ rank(unknown)`。
//! 3. **机制的覆盖范围必须与其声明的范围一致；不一致处必须显式登记。**
//!    R005 的判据范围**声明为**"`RiskLevel` 词表"（注册表 §1.2），
//!    机制**就是**逐行读那张表 ⇒ 二者一致。已知的两个缺口（沉默/空串类型上不可分、
//!    "零字面量"无独立检查器）**显式登记在注册表 §1.2 与 §6.6/§6.7**，不藏在这里。
//!
//! # 词表从哪来
//!
//! `RISK_VOCABULARY` 由 `core/build.rs` 在构建期从
//! `commonintents/.github/CI-144_码注册表.md` §1.2 派生。本文件**不含任何等级字面量** ——
//! 这正是 K-104(a)（手维护清单落在后面）的修法：一个来源，不是两份。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// 词表（构建期从 CI-144 码注册表派生）
// ============================================================================

mod registry {
    include!(concat!(env!("OUT_DIR"), "/risk_vocabulary.rs"));
}

/// 与 `build.rs` **共用**的解析器：测试期用它复核"生成的词表 == 注册表现状"。
///
/// 共用是刻意的：解析逻辑只有一份，否则"漂移检查"自己就会有第二份实现，
/// 而两份实现之间的漂移正是它要防的东西。
///
/// 它在库里**只在测试期**需要（生产路径只用构建期生成的常量），所以 `cfg(test)`；
/// `build.rs` 是另一份编译单元，用 `include!` 直接拿同一份源码。
#[cfg(test)]
#[path = "registry_parse.rs"]
mod registry_parse;

/// 注册表派生的一行（列序与注册表 §1.2 一致）。
pub use registry::RiskVocabRow;
/// 生成词表时读到的注册表文件路径（审计用）。
pub use registry::REGISTRY_SOURCE;
/// `RiskLevel` 词表本体 —— R005 的唯一判据来源。
pub use registry::RISK_VOCABULARY;

/// 取某一类别的唯一一行。
///
/// 构建期已校验"四类判定出口各恰好一行"，所以这里取不到就是**构建期不变量被破坏**；
/// 直接失败而不是回退到一个默认类别 —— 回退就是"用默认值承载未知"。
fn row_of_class(class: &str) -> &'static RiskVocabRow {
    RISK_VOCABULARY
        .iter()
        .find(|r| r.class == class)
        .unwrap_or_else(|| {
            panic!(
                "词表不变量被破坏：class={class:?} 没有行（build.rs 本应拦住）—— 拒绝给结论"
            )
        })
}

// ============================================================================
// 审查结果类型
// ============================================================================

/// 审查严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// 信息（不阻塞，仅提示）
    Info,
    /// 警告（不阻塞，但建议修复）
    Warning,
    /// 错误（阻塞，必须修复）
    Error,
}

impl Default for Severity {
    fn default() -> Self {
        Severity::Info
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
        }
    }
}

/// 处置：判完之后**接下来怎么办**（与 [`Severity`] 正交 —— 严重度是"多重"，
/// 处置是"落到哪"，而报告要能同时回答这两个问题）。
///
/// 三档来自 `CI-144_跨链生命周期与反馈契约.md` §12.4，可接受度与之单调：
/// `Accepted`（词表内的已知等级）< `Quarantine`（`unknown` / 保留段）< `Rejected`（沉默 / 词表外）。
///
/// 落盘映射（由学习器的 `ReviewPipeline` 执行，本 crate 不做状态迁移）：
/// `Quarantine` ⇒ `Warning` ⇒ `staging/` ｜ `Rejected` ⇒ `Error` ⇒ `rejected/`。
/// **`rejected/` 本身就是隔离区**（不是删除）：manifest 原样保留在那里，
/// 可人工查看、可重放（重放 = 对同一份 manifest 再跑一次 `review()`，判定是确定性的）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    /// 通过：值在词表内且是"已知"等级
    Accepted,
    /// 隔离待人工复核（`staging/`）
    Quarantine,
    /// 拒绝（`rejected/`）—— `rejected/` 也是隔离区，不删除任何东西
    Rejected,
}

impl Disposition {
    /// 可接受度序：**越小越宽松**。单调性断言用这个序，而不是靠"看起来更重"。
    pub fn acceptance_rank(&self) -> u8 {
        match self {
            Disposition::Accepted => 0,
            Disposition::Quarantine => 1,
            Disposition::Rejected => 2,
        }
    }
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Disposition::Accepted => write!(f, "ACCEPTED"),
            Disposition::Quarantine => write!(f, "QUARANTINE"),
            Disposition::Rejected => write!(f, "REJECTED"),
        }
    }
}

/// 一次 `RiskLevel` 判定的完整结果：**合法性**（认不认）与**处置**（怎么办）分开，
/// 且原值原样携带。
///
/// 这是"未知必须显式表示"的落点：判定结果里**没有**"缺省"这一栏 ——
/// 每一种输入都被映射到某一类，且类别名本身说明它是什么（`silence` 不叫 `low`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskVerdict {
    /// 原样保留的输入（不归一化、不丢弃）—— 注册表 §4.2「未知码必须可被原样携带」
    pub raw: String,
    /// 命中的注册表类别：`value` / `unknown` / `silence` / `illegal` / `reserved`
    pub class: &'static str,
    /// 处置
    pub disposition: Disposition,
    /// 若产生发现，其严重度（`Accepted` 为 `None` —— 显式没有，不是"默认 Info"）
    pub severity: Option<Severity>,
    /// 注册表诊断码（`Accepted` 为 `None`）—— 可进证轨、可计数（契约 §6）
    pub code: Option<&'static str>,
    /// 注册表的「确定性后果」原文（给机器看的结论，不是渲染）
    pub consequence: &'static str,
    /// 该值是否要求 R008 的「高风险细节」
    pub high_risk: bool,
}

/// 按 **CI-144 码注册表 §1.2** 判定一个 `risk_level` 输入。
///
/// 判定顺序（每一步都只有一个出口，不存在"掉到默认分支"）：
///
/// | 输入 | class | 处置 | 严重度 |
/// |---|---|---|---|
/// | 词表 `value` 行（low/medium/critical/catastrophic…改注册表即改这里） | `value` | `Accepted` | — |
/// | `unknown` 行 | `unknown` | `Quarantine` | `Warning` |
/// | 空 / 只有空白 | `silence` | `Rejected` | `Error` |
/// | 保留段前缀（`X-`） | `reserved` | `Quarantine` | `Warning` |
/// | 其余 | `illegal` | `Rejected` | `Error` |
///
/// **本函数里没有任何等级字面量** —— 表在注册表里，加一行就多认一个值（见 ③ 的测试）。
pub fn classify_risk_level(raw: &str) -> RiskVerdict {
    let probe = raw.trim().to_lowercase();
    let row: &'static RiskVocabRow = if probe.is_empty() {
        // 「沉默」不是一种声明：把它与"声明 unknown"分开，且不得更轻。
        row_of_class("silence")
    } else if let Some(v) = RISK_VOCABULARY
        .iter()
        .find(|r| r.class == "value" && r.value.to_lowercase() == probe)
    {
        v
    } else if let Some(u) = RISK_VOCABULARY
        .iter()
        .find(|r| r.class == "unknown" && r.value.to_lowercase() == probe)
    {
        u
    } else {
        // 保留段：前缀匹配。保留码**永不赋予语义**，但必须可被原样携带且不被拒（§5 grease）。
        let reserved = row_of_class("reserved");
        if !reserved.value.is_empty() && probe.starts_with(&reserved.value.to_lowercase()) {
            reserved
        } else {
            row_of_class("illegal")
        }
    };

    let disposition = match row.class {
        "value" => Disposition::Accepted,
        "unknown" | "reserved" => Disposition::Quarantine,
        _ => Disposition::Rejected,
    };
    let (severity, code) = match disposition {
        Disposition::Accepted => (None, None),
        Disposition::Quarantine => (Some(Severity::Warning), Some(row.code)),
        Disposition::Rejected => (Some(Severity::Error), Some(row.code)),
    };

    RiskVerdict {
        raw: raw.to_string(),
        class: row.class,
        disposition,
        severity,
        code,
        consequence: row.consequence,
        high_risk: row.high_risk,
    }
}

/// 要求 R008「高风险细节」的等级集合（**同样从注册表派生**，不是第二处清单）。
pub fn high_risk_values() -> Vec<&'static str> {
    RISK_VOCABULARY
        .iter()
        .filter(|r| r.high_risk)
        .map(|r| r.value)
        .collect()
}

/// 单个审查发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// 规则 ID（如 "R001"）
    pub rule_id: String,
    /// 严重级别
    pub severity: Severity,
    /// 发现描述
    pub message: String,
    /// **注册表诊断码**（如 `E-RISK-MISSING`）。
    ///
    /// `None` = 该规则**尚无已登记的码**（契约 §3 规则 3 要求"新增码先登记后使用"）。
    /// 这是一个**显式登记的缺口**（注册表 §6.1：各器官自有诊断码尚未清点），
    /// 而不是"用默认值冒充一个码"。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// 相关字段（如 "name"、"parameters_schema"）
    #[serde(default)]
    pub field: Option<String>,
    /// 修复建议
    #[serde(default)]
    pub suggestion: Option<String>,
}

/// 审查报告
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewReport {
    /// 被审查的工具名称
    pub tool_name: String,
    /// 被审查的工具版本
    pub tool_version: String,
    /// 审查规则版本
    pub rules_version: String,
    /// 审查发现列表
    pub findings: Vec<ReviewFinding>,
    /// 审查时间（Unix 时间戳）
    pub reviewed_at: String,
}

impl ReviewReport {
    /// 是否通过审查（无 Error 级别的发现）
    pub fn passed(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// 获取 Error 级别的发现
    pub fn errors(&self) -> Vec<&ReviewFinding> {
        self.findings.iter().filter(|f| f.severity == Severity::Error).collect()
    }

    /// 获取 Warning 级别的发现
    pub fn warnings(&self) -> Vec<&ReviewFinding> {
        self.findings.iter().filter(|f| f.severity == Severity::Warning).collect()
    }

    /// 按**注册表诊断码**计数（契约 §6「每个码进证轨 + 可计数」）。
    ///
    /// 这是本仓库里**唯一存在的"门铃"**：`HelixECO-Glove` 没有通知机制、没有 CLI，
    /// `core` 也没有 `tracing` 依赖。因此"被隔离/被拒"要被人看见，只能靠：
    /// 1. 发现里带**已登记的码**（[`ReviewFinding::code`]），本函数把它变成计数；
    /// 2. 报告被**落盘**（学习器的 `ReviewPipeline::process_and_write` 写
    ///    `<server>_review_report.json`，并把 manifest 原样写进 `rejected/`、`staging/`）；
    /// 3. 学习器对该文件的 `tracing::info!` 状态迁移日志。
    ///
    /// **重放**：判定是纯函数且确定性的 —— 对 `rejected/` 里那份 manifest 再跑一次
    /// `review()`（或再跑一次管道）会得到同一个码与同一个处置，所以人工修好后可以安全重放。
    /// 未登记码的发现（见 [`ReviewFinding::code`]）不计入本表 —— 那是显式登记的缺口，
    /// 而不是"默认为 0"。
    pub fn codes(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for finding in &self.findings {
            if let Some(code) = &finding.code {
                *map.entry(code.clone()).or_insert(0) += 1;
            }
        }
        map
    }

    /// R005 判定出的处置集合（按发现顺序；`Accepted` 不产生发现，故不出现）。
    ///
    /// 这是"门铃"的**人可读**侧：`Quarantine` 走 `staging/`、`Rejected` 走 `rejected/`，
    /// 两者都保留 manifest 原文件，都不删除任何东西。
    pub fn risk_dispositions(&self) -> Vec<Disposition> {
        self.findings
            .iter()
            .filter(|f| f.rule_id == "R005")
            .map(|f| match f.severity {
                Severity::Warning => Disposition::Quarantine,
                Severity::Error => Disposition::Rejected,
                Severity::Info => Disposition::Accepted,
            })
            .collect()
    }

    /// 统计各严重级别的数量
    pub fn summary(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for finding in &self.findings {
            let key = finding.severity.to_string();
            *map.entry(key).or_insert(0) += 1;
        }
        map
    }
}

// ============================================================================
/// 工具 Manifest 的最小子集（用于审查）
///
/// 不依赖 tentacle-core，保持 core 纯净。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolManifest {
    /// 工具名称（点分命名空间：<platform>.<domain>.<action>）
    pub name: String,
    /// 工具版本（SemVer 格式）
    pub version: String,
    /// 工具描述
    pub description: String,
    /// 参数 Schema（JSON Schema 格式）
    #[serde(default)]
    pub parameters_schema: serde_json::Value,
    /// 风险等级（low / medium / critical / catastrophic）
    #[serde(default)]
    pub risk_level: String,
    /// 平台标识（macos / linux / generic 等）
    #[serde(default)]
    pub platform: String,
    /// 支持的宿主 OS 列表（空列表表示支持所有平台）
    #[serde(default)]
    pub host_os: Vec<String>,
    /// 标签列表
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ToolManifest {
    /// 从 JSON 字符串解析
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 从文件读取
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

// ============================================================================
// 审查规则
// ============================================================================

/// 审查规则版本
pub const RULES_VERSION: &str = "1.0.0";

/// 静态审查器（L1）
pub struct StaticReviewer {
    rules_version: String,
}

impl Default for StaticReviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticReviewer {
    /// 创建新的静态审查器
    pub fn new() -> Self {
        Self {
            rules_version: RULES_VERSION.to_string(),
        }
    }

    /// 审查工具 Manifest
    pub fn review(&self, manifest: &ToolManifest) -> ReviewReport {
        let mut findings = Vec::new();

        // R001: 名称必须非空且符合命名规范
        self.check_name(manifest, &mut findings);

        // R002: 版本必须符合 SemVer
        self.check_version(manifest, &mut findings);

        // R003: 描述必须非空
        self.check_description(manifest, &mut findings);

        // R004: 参数 Schema 必须是合法的 JSON Schema
        self.check_parameters_schema(manifest, &mut findings);

        // R005: 风险等级必须合法
        self.check_risk_level(manifest, &mut findings);

        // R006: 平台标识必须合法
        self.check_platform(manifest, &mut findings);

        // R007: host_os 列表必须合法（如果指定）
        self.check_host_os(manifest, &mut findings);

        // R008: 高风险工具必须有明确的描述和参数说明
        self.check_high_risk_details(manifest, &mut findings);

        // R009: 工具名称必须符合点分命名空间规范
        self.check_name_namespace(manifest, &mut findings);

        ReviewReport {
            tool_name: manifest.name.clone(),
            tool_version: manifest.version.clone(),
            rules_version: self.rules_version.clone(),
            findings,
            reviewed_at: chrono_now(),
        }
    }

    /// 批量审查多个 Manifest
    pub fn review_batch(&self, manifests: &[ToolManifest]) -> Vec<ReviewReport> {
        manifests.iter().map(|m| self.review(m)).collect()
    }

    // --- 规则实现 ---

    fn check_name(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.name.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R001".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Error,
                message: "工具名称不能为空".to_string(),
                field: Some("name".to_string()),
                suggestion: Some("提供一个描述性的工具名称".to_string()),
            });
        }
    }

    fn check_name_namespace(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.name.is_empty() {
            return;
        }
        // 点分命名空间：<platform>.<domain>.<action>
        let parts: Vec<&str> = manifest.name.split('.').collect();
        if parts.len() < 2 {
            findings.push(ReviewFinding {
                rule_id: "R009".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Warning,
                message: format!(
                    "工具名称 '{}' 不符合点分命名空间规范（建议：<platform>.<domain>.<action>）",
                    manifest.name
                ),
                field: Some("name".to_string()),
                suggestion: Some("例如：macos.fs.read_file".to_string()),
            });
        }
    }

    fn check_version(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.version.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R002".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Warning,
                message: "工具版本为空".to_string(),
                field: Some("version".to_string()),
                suggestion: Some("使用 SemVer 格式，如 1.0.0".to_string()),
            });
            return;
        }
        // 简单的 SemVer 检查
        let parts: Vec<&str> = manifest.version.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
            findings.push(ReviewFinding {
                rule_id: "R002".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Warning,
                message: format!("版本 '{}' 不符合 SemVer 格式", manifest.version),
                field: Some("version".to_string()),
                suggestion: Some("使用 X.Y.Z 格式，如 1.0.0".to_string()),
            });
        }
    }

    fn check_description(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.description.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R003".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Error,
                message: "工具描述不能为空".to_string(),
                field: Some("description".to_string()),
                suggestion: Some("提供工具功能的清晰描述".to_string()),
            });
        } else if manifest.description.len() < 10 {
            findings.push(ReviewFinding {
                rule_id: "R003".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Warning,
                message: "工具描述过短（少于 10 字符）".to_string(),
                field: Some("description".to_string()),
                suggestion: Some("提供更详细的功能描述".to_string()),
            });
        }
    }

    fn check_parameters_schema(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.parameters_schema.is_null() {
            findings.push(ReviewFinding {
                rule_id: "R004".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Info,
                message: "工具没有参数 Schema（无参数工具）".to_string(),
                field: Some("parameters_schema".to_string()),
                suggestion: None,
            });
            return;
        }

        // 检查是否有 type 字段
        if let Some(obj) = manifest.parameters_schema.as_object() {
            if !obj.contains_key("type") {
                findings.push(ReviewFinding {
                    rule_id: "R004".to_string(),
                    code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                    severity: Severity::Warning,
                    message: "参数 Schema 缺少 'type' 字段".to_string(),
                    field: Some("parameters_schema".to_string()),
                    suggestion: Some("添加 \"type\": \"object\" 或其他合法类型".to_string()),
                });
            }
            // 检查 properties 或 items
            if !obj.contains_key("properties") && !obj.contains_key("items") {
                findings.push(ReviewFinding {
                    rule_id: "R004".to_string(),
                    code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                    severity: Severity::Warning,
                    message: "参数 Schema 缺少 'properties' 或 'items' 字段".to_string(),
                    field: Some("parameters_schema".to_string()),
                    suggestion: Some("定义参数的属性结构".to_string()),
                });
            }
        } else {
            findings.push(ReviewFinding {
                rule_id: "R004".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Error,
                message: "参数 Schema 不是合法的 JSON 对象".to_string(),
                field: Some("parameters_schema".to_string()),
                suggestion: Some("使用 JSON Schema 格式定义参数".to_string()),
            });
        }
    }

    /// R005 —— 风险等级必须**可判定**。
    ///
    /// 修复前的两处错（`CI-144_跨链生命周期与反馈契约.md` §12.2 已实测）：
    ///
    /// - **分级是反的**：`is_empty()` ⇒ `Warning`（轻），`"UNKNOWN"` ⇒ `Error`（重）。
    ///   于是**沉默比诚实声明更被宽恕** —— 最便宜的攻击不是伪造危险等级，而是**删掉这个字段**。
    /// - **合法集是手维护的**：`let valid = [...]` 是第二处清单，学习器刚加的
    ///   `RiskLevel::Unknown` 不在里面 ⇒ 声明"我不知道"反而落 `rejected/`，
    ///   而那会**摧毁 `FieldProvenance`（来源/规则/信任度）**：留空就没有出处可丢。
    ///
    /// 现在判定完全由 `classify_risk_level`（词表来自注册表）给出，方向由 §12.4 的
    /// 三档单调性钉住：`Accepted > Quarantine ≥ Rejected`，且**沉默不得轻于 `unknown`**。
    /// `unknown` 走 `Quarantine`（`staging/`，人工复核）而不是 `rejected/`；
    /// `rejected/` 对"沉默"与"词表外的值"仍然敞开，且**两者各有自己的码**，不混为一谈。
    fn check_risk_level(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        let verdict = classify_risk_level(&manifest.risk_level);
        // `Accepted` 显式没有严重度 —— 这里不是一个"默认放行"的分支，而是"无发现"。
        let Some(severity) = verdict.severity else {
            return;
        };
        let code = verdict
            .code
            .expect("非 Accepted 的判定必须带注册码（词表校验保证）");
        let (message, suggestion) = match verdict.class {
            "unknown" => (
                format!(
                    "风险等级 '{}' 是**声明不知道**：已隔离待人工复核（{}）—— 不是拒绝，也不是沉默",
                    verdict.raw, verdict.consequence
                ),
                "人工复核后改判；或修正 manifest 后重放 review()（判定是确定性的）。\
                 注意：把字段留空/删掉不是更好的选择 —— 沉默会被拒绝。"
                    .to_string(),
            ),
            "silence" => (
                format!(
                    "风险等级**缺失或为空**（沉默）：已拒绝（{}）—— 沉默不是一种声明，\
                     它比显式写 unknown 信息更少，因此不得更轻",
                    verdict.consequence
                ),
                format!(
                    "补上风险等级；若确实判不出，**显式写 `{}`**（会被隔离复核，不会被拒）。\
                     已保留原值：'{}'",
                    row_of_class("unknown").value,
                    verdict.raw
                ),
            ),
            "reserved" => (
                format!(
                    "风险等级 '{}' 落在保留段 `{}`（永不赋予语义）：已隔离（{}）",
                    verdict.raw,
                    row_of_class("reserved").value,
                    verdict.consequence
                ),
                "保留段值是 grease 探针用的：本版本不认识它，但必须容得下它。\
                 请人工确认这是探针还是写错了。"
                    .to_string(),
            ),
            _ => (
                format!(
                    "风险等级 '{}' 不在注册表词表内（{}）：已拒绝",
                    verdict.raw, verdict.consequence
                ),
                format!(
                    "使用注册表 §1.2 里的值（当前已知：{}）；\
                     或先在 `CI-144_码注册表.md` §1.2 登记新值。已保留原值：'{}'",
                    RISK_VOCABULARY
                        .iter()
                        .filter(|r| r.class == "value")
                        .map(|r| r.value)
                        .collect::<Vec<_>>()
                        .join(" / "),
                    verdict.raw
                ),
            ),
        };
        findings.push(ReviewFinding {
            rule_id: "R005".to_string(),
            severity,
            message,
            code: Some(code.to_string()),
            field: Some("risk_level".to_string()),
            suggestion: Some(suggestion),
        });
    }

    fn check_platform(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.platform.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R006".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Info,
                message: "平台标识未指定（默认为通用工具）".to_string(),
                field: Some("platform".to_string()),
                suggestion: None,
            });
            return;
        }
        let valid = [
            "macos", "linux", "windows", "harmony", "android", "ios",
            "robot", "generic", "web", "cloud",
        ];
        if !valid.contains(&manifest.platform.to_lowercase().as_str()) {
            findings.push(ReviewFinding {
                rule_id: "R006".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Warning,
                message: format!("平台标识 '{}' 不在已知列表中", manifest.platform),
                field: Some("platform".to_string()),
                suggestion: Some("使用已知平台标识，或确认这是新平台".to_string()),
            });
        }
    }

    fn check_host_os(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.host_os.is_empty() {
            return; // 空列表表示支持所有平台
        }
        let valid = ["macos", "linux", "windows", "ios", "android", "freebsd"];
        for os in &manifest.host_os {
            if !valid.contains(&os.to_lowercase().as_str()) {
                findings.push(ReviewFinding {
                    rule_id: "R007".to_string(),
                    code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                    severity: Severity::Warning,
                    message: format!("host_os '{}' 不在已知列表中", os),
                    field: Some("host_os".to_string()),
                    suggestion: Some("使用已知 OS 标识".to_string()),
                });
            }
        }
    }

    fn check_high_risk_details(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        // 集合从注册表派生（`high_risk` 列），不是第二处手维护的 `["critical","catastrophic"]`。
        // `unknown` 也在列（无证据 ⇒ 必须人工确认，与学习器 `requires_confirmation` 同向）。
        let verdict = classify_risk_level(&manifest.risk_level);
        if !verdict.high_risk {
            return;
        }
        // 高风险工具必须有详细描述
        if manifest.description.len() < 20 {
            findings.push(ReviewFinding {
                rule_id: "R008".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Error,
                message: "高风险工具必须有详细描述（至少 20 字符）".to_string(),
                field: Some("description".to_string()),
                suggestion: Some("详细说明工具的用途、风险和安全措施".to_string()),
            });
        }
        // 高风险工具必须有参数 Schema
        if manifest.parameters_schema.is_null() {
            findings.push(ReviewFinding {
                rule_id: "R008".to_string(),
                code: None, // 该规则的码尚未登记（注册表 §6.1 已显式登记此缺口）
                severity: Severity::Error,
                message: "高风险工具必须定义参数 Schema".to_string(),
                field: Some("parameters_schema".to_string()),
                suggestion: Some("定义所有输入参数的类型和约束".to_string()),
            });
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

fn chrono_now() -> String {
    // 不依赖 chrono crate，使用标准库
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> ToolManifest {
        ToolManifest {
            name: "macos.fs.read_file".to_string(),
            version: "1.0.0".to_string(),
            description: "Read a file from the filesystem and return its content".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to read"}
                },
                "required": ["path"]
            }),
            risk_level: "low".to_string(),
            platform: "macos".to_string(),
            host_os: vec!["macos".to_string()],
            tags: vec!["filesystem".to_string(), "read".to_string()],
        }
    }

    #[test]
    fn test_valid_manifest_passes() {
        let reviewer = StaticReviewer::new();
        let manifest = valid_manifest();
        let report = reviewer.review(&manifest);
        assert!(report.passed(), "Valid manifest should pass: {:?}", report.findings);
    }

    #[test]
    fn test_empty_name_fails() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.name = String::new();
        let report = reviewer.review(&manifest);
        assert!(!report.passed());
        assert!(report.errors().iter().any(|f| f.rule_id == "R001"));
    }

    #[test]
    fn test_empty_description_fails() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.description = String::new();
        let report = reviewer.review(&manifest);
        assert!(!report.passed());
        assert!(report.errors().iter().any(|f| f.rule_id == "R003"));
    }

    #[test]
    fn test_invalid_risk_level_fails() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.risk_level = "extreme".to_string();
        let report = reviewer.review(&manifest);
        assert!(!report.passed());
        assert!(report.errors().iter().any(|f| f.rule_id == "R005"));
    }

    #[test]
    fn test_high_risk_requires_details() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.risk_level = "critical".to_string();
        manifest.description = "short".to_string();
        let report = reviewer.review(&manifest);
        assert!(!report.passed());
        assert!(report.errors().iter().any(|f| f.rule_id == "R008"));
    }

    #[test]
    fn test_non_namespace_name_warns() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.name = "readfile".to_string();
        let report = reviewer.review(&manifest);
        assert!(report.passed()); // 只是 warning，不阻塞
        assert!(report.warnings().iter().any(|f| f.rule_id == "R009"));
    }

    #[test]
    fn test_invalid_version_warns() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.version = "v1".to_string();
        let report = reviewer.review(&manifest);
        assert!(report.passed()); // 只是 warning
        assert!(report.warnings().iter().any(|f| f.rule_id == "R002"));
    }

    #[test]
    fn test_report_summary() {
        let reviewer = StaticReviewer::new();
        let manifest = valid_manifest();
        let report = reviewer.review(&manifest);
        let summary = report.summary();
        // 合法 manifest 应该没有 error 和 warning
        assert_eq!(summary.get("ERROR"), None);
        assert_eq!(summary.get("WARNING"), None);
    }

    #[test]
    fn test_batch_review() {
        let reviewer = StaticReviewer::new();
        let manifests = vec![valid_manifest(), valid_manifest()];
        let reports = reviewer.review_batch(&manifests);
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().all(|r| r.passed()));
    }

    #[test]
    fn test_no_parameters_schema_is_info() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.parameters_schema = serde_json::Value::Null;
        let report = reviewer.review(&manifest);
        assert!(report.passed());
        assert!(report.findings.iter().any(|f| f.rule_id == "R004" && f.severity == Severity::Info));
    }

    // ------------------------------------------------------------------
    // ③ 派生性正控：注册表加一个值 ⇒ R005 自动接受，**不改 R005 代码**
    // ------------------------------------------------------------------

    /// ③ 对**当前注册表里的每一个** `value` 行，R005 都必须接受。
    ///
    /// 这条测试**不写死任何值** —— 它迭代的是构建期从注册表派生的表。
    /// 于是：往 `CI-144_码注册表.md` §1.2 加一行 `| elevated | value | … |`，
    /// 重新构建后这条断言自动多一行，而 `reviewer.rs` **一个字都不用改**。
    /// 反过来说，只要 R005 里还藏着第二处手维护清单，新值就会在这里变红。
    #[test]
    fn c3_every_registry_value_is_accepted_with_no_r005_code_change() {
        let values: Vec<&RiskVocabRow> =
            RISK_VOCABULARY.iter().filter(|r| r.class == "value").collect();
        // 先防"空集造成的假绿"：一张空词表会让下面这个循环什么都不查就通过。
        assert!(
            !values.is_empty(),
            "③ 词表里一个 value 行都没有 —— 这条测试会变成空转的假绿"
        );
        for row in values {
            let verdict = classify_risk_level(row.value);
            assert_eq!(
                verdict.disposition,
                Disposition::Accepted,
                "③ 注册表说 {:?} 合法，R005 却不接受 —— 派生是假的",
                row.value
            );
            let report = StaticReviewer::new().review(&ToolManifest {
                risk_level: row.value.to_string(),
                ..valid_manifest()
            });
            assert!(
                !report.findings.iter().any(|f| f.rule_id == "R005"),
                "③ 注册表里的合法值 {:?} 仍被 R005 拦下",
                row.value
            );
        }
        // ③ 的第二个正控：注册表里**登记了码**的每一类都得真的用上那个码。
        for row in RISK_VOCABULARY.iter().filter(|r| r.code != "-") {
            let probe = match row.class {
                "unknown" => row.value.to_string(),
                "reserved" => format!("{}PROBE", row.value),
                "silence" => String::new(),
                _ => "definitely-not-in-the-registry".to_string(),
            };
            let verdict = classify_risk_level(&probe);
            assert_eq!(
                verdict.code,
                Some(row.code),
                "③ {:?} 类应当产生注册码 {:?}，实际 {:?}",
                row.class,
                row.code,
                verdict.code
            );
        }
    }

    /// 漂移门：**构建期生成的词表**必须与**现在读到的注册表**逐行相同。
    ///
    /// 它和 `build.rs` 共用同一份解析器（`registry_parse`），所以"检查器自己漂移"
    /// 这件事不可能发生。若它红了：注册表变了而构建没重来 —— 重新构建即可
    /// （`build.rs` 已 `rerun-if-changed` 指向注册表）。
    #[test]
    fn test_registry_vocabulary_is_not_stale() {
        let text = std::fs::read_to_string(REGISTRY_SOURCE).unwrap_or_else(|e| {
            panic!(
                "读不到构建期用的注册表 {}: {e} —— 词表来源必须始终可达",
                REGISTRY_SOURCE
            )
        });
        let live = super::registry_parse::parse(&text)
            .unwrap_or_else(|e| panic!("注册表 §1.2 现在解析不了：{e}"));
        assert_eq!(
            live.len(),
            RISK_VOCABULARY.len(),
            "注册表行数({}) != 构建期词表行数({})：词表已漂移，请重新构建",
            live.len(),
            RISK_VOCABULARY.len()
        );
        for (a, b) in live.iter().zip(RISK_VOCABULARY.iter()) {
            assert_eq!(
                (
                    a.value.as_str(),
                    a.class.as_str(),
                    a.code.as_str(),
                    a.high_risk
                ),
                (b.value, b.class, b.code, b.high_risk),
                "注册表 {} 与构建期词表不一致（漂移）",
                a.value
            );
        }
    }

    /// 词表**只有一份**、且**不含等级字面量**的窄守卫（注册表 §6.7 已登记：
    /// 这只是源码级 grep，不是通用检查器）。
    ///
    /// 它挡住的是"修复被回退"这个具体形态：把 `let valid = [...]` 或
    /// `matches!(…, "critical" | "catastrophic")` 抄回生产代码。
    /// 只扫**生产段**的非注释行 —— 文档里当然要能引用旧代码来记账。
    #[test]
    fn test_reviewer_source_has_no_hand_maintained_risk_list() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/reviewer.rs"),
        )
        .expect("读自己的源码");
        // 测试段当然会写死值来构造用例，所以只看 `#[cfg(test)]` 之前的生产段。
        let production = src.split("#[cfg(test)]").next().unwrap_or("");
        for (i, line) in production.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue; // 注释/文档可以引用旧代码（记账需要）
            }
            for banned in ["\"critical\"", "\"catastrophic\""] {
                assert!(
                    !line.contains(banned),
                    "第 {} 行生产代码里出现了等级字面量 {banned} —— 词表只能来自注册表：{line}",
                    i + 1
                );
            }
        }
    }

    /// R008 的高风险集合也必须派生（否则就是"修了 R005、漏了 R008 的手维护清单"）。
    #[test]
    fn test_high_risk_set_is_derived_from_registry() {
        let derived = high_risk_values();
        let expected: Vec<&str> = RISK_VOCABULARY
            .iter()
            .filter(|r| r.high_risk)
            .map(|r| r.value)
            .collect();
        assert_eq!(derived, expected);
        assert!(derived.contains(&"critical") && derived.contains(&"catastrophic"));
        // `unknown` 无证据 ⇒ 必须人工确认（与学习器 `requires_confirmation` 同向）
        assert!(derived.contains(&"unknown"));
        assert!(!derived.contains(&"low") && !derived.contains(&"medium"));
    }

    // ------------------------------------------------------------------
    // B. 隔离不是删除：可计数（门铃）+ 可找到 + 可重放
    // ------------------------------------------------------------------

    /// 隔离/拒绝必须**可见**：发现要带已登记的码，且码可计数（契约 §6）。
    ///
    /// 本仓库**没有通知机制**（没有 notifier、没有 CLI，`core` 连 `tracing` 都没有），
    /// 所以"门铃"只能是：已登记的码 + 计数 + 报告落盘 + 学习器的迁移日志。
    /// 这条测试钉住其中能在这里钉的部分，其余在 [`ReviewReport::codes`] 的文档里点名。
    #[test]
    fn test_quarantine_is_countable_and_coded() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.risk_level = "UNKNOWN".to_string();
        let report = reviewer.review(&manifest);

        assert_eq!(report.risk_dispositions(), vec![Disposition::Quarantine]);
        let codes = report.codes();
        assert_eq!(
            codes.get("W-RISK-UNKNOWN"),
            Some(&1),
            "隔离必须带注册码才可计数，实际：{codes:?}"
        );
        assert_eq!(report.warnings().len(), 1);
        assert_eq!(report.errors().len(), 0);

        // 沉默：同一条门铃的另一档，码不同 —— 两者不混为一谈
        let mut silent = valid_manifest();
        silent.risk_level = String::new();
        let report = reviewer.review(&silent);
        assert_eq!(report.risk_dispositions(), vec![Disposition::Rejected]);
        assert_eq!(report.codes().get("E-RISK-MISSING"), Some(&1));
    }

    /// 被隔离的东西必须**可被找到，且可被重放**。
    ///
    /// 重放 = 对同一份 manifest 再跑一次 `review()`。判定是纯函数，所以结果确定：
    /// 人工复核后无论"改判"还是"改 manifest 再跑"，都不会得到一个不同的结论。
    #[test]
    fn test_quarantine_is_replayable_after_serialization() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.risk_level = "UNKNOWN".to_string();
        let first = reviewer.review(&manifest);

        // 落盘 → 读回（学习器把 manifest 原样写进 staging/，人工据此重放）
        let json = serde_json::to_string(&manifest).expect("manifest 可序列化");
        let restored: ToolManifest = serde_json::from_str(&json).expect("manifest 可读回");
        let replayed = reviewer.review(&restored);

        assert_eq!(first.codes(), replayed.codes(), "重放必须得到同一个码");
        assert_eq!(
            first.risk_dispositions(),
            replayed.risk_dispositions(),
            "重放必须得到同一个处置"
        );
        assert_eq!(first.findings.len(), replayed.findings.len());
    }

    /// 报告的 JSON 形状向后兼容：**旧报告没有 `code` 键**，读回来必须是 `None`
    /// （显式表示"没有码"），而不是某个默认码。
    #[test]
    fn test_legacy_report_without_code_deserializes_to_none() {
        let legacy = r#"{
            "rule_id": "R001",
            "severity": "error",
            "message": "工具名称不能为空",
            "field": "name"
        }"#;
        let f: ReviewFinding = serde_json::from_str(legacy).expect("旧报告必须仍可解析");
        assert_eq!(f.code, None, "缺失的码必须读成 None，不得被默认值填充");
        assert_eq!(f.field.as_deref(), Some("name"));
    }

    /// serde 往返不丢码（证轨里必须留得住）。
    #[test]
    fn test_code_survives_serde_roundtrip() {
        let reviewer = StaticReviewer::new();
        let mut manifest = valid_manifest();
        manifest.risk_level = "X-PROBE".to_string();
        let report = reviewer.review(&manifest);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("W-RISK-RESERVED"), "码必须落进 JSON：{json}");
        let back: ReviewReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.codes(), report.codes());
    }
}
