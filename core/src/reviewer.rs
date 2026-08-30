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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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

/// 单个审查发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// 规则 ID（如 "R001"）
    pub rule_id: String,
    /// 严重级别
    pub severity: Severity,
    /// 发现描述
    pub message: String,
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
                severity: Severity::Error,
                message: "工具描述不能为空".to_string(),
                field: Some("description".to_string()),
                suggestion: Some("提供工具功能的清晰描述".to_string()),
            });
        } else if manifest.description.len() < 10 {
            findings.push(ReviewFinding {
                rule_id: "R003".to_string(),
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
                    severity: Severity::Warning,
                    message: "参数 Schema 缺少 'properties' 或 'items' 字段".to_string(),
                    field: Some("parameters_schema".to_string()),
                    suggestion: Some("定义参数的属性结构".to_string()),
                });
            }
        } else {
            findings.push(ReviewFinding {
                rule_id: "R004".to_string(),
                severity: Severity::Error,
                message: "参数 Schema 不是合法的 JSON 对象".to_string(),
                field: Some("parameters_schema".to_string()),
                suggestion: Some("使用 JSON Schema 格式定义参数".to_string()),
            });
        }
    }

    fn check_risk_level(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.risk_level.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R005".to_string(),
                severity: Severity::Warning,
                message: "风险等级未指定".to_string(),
                field: Some("risk_level".to_string()),
                suggestion: Some("指定风险等级：low / medium / critical / catastrophic".to_string()),
            });
            return;
        }
        let valid = ["low", "medium", "critical", "catastrophic"];
        if !valid.contains(&manifest.risk_level.to_lowercase().as_str()) {
            findings.push(ReviewFinding {
                rule_id: "R005".to_string(),
                severity: Severity::Error,
                message: format!("风险等级 '{}' 不合法", manifest.risk_level),
                field: Some("risk_level".to_string()),
                suggestion: Some("使用：low / medium / critical / catastrophic".to_string()),
            });
        }
    }

    fn check_platform(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        if manifest.platform.is_empty() {
            findings.push(ReviewFinding {
                rule_id: "R006".to_string(),
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
                    severity: Severity::Warning,
                    message: format!("host_os '{}' 不在已知列表中", os),
                    field: Some("host_os".to_string()),
                    suggestion: Some("使用已知 OS 标识".to_string()),
                });
            }
        }
    }

    fn check_high_risk_details(&self, manifest: &ToolManifest, findings: &mut Vec<ReviewFinding>) {
        let is_high = matches!(
            manifest.risk_level.to_lowercase().as_str(),
            "critical" | "catastrophic"
        );
        if !is_high {
            return;
        }
        // 高风险工具必须有详细描述
        if manifest.description.len() < 20 {
            findings.push(ReviewFinding {
                rule_id: "R008".to_string(),
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
}
