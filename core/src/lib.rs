//! Helix ECO Glove 标准接口定义
//!
//! 本 crate 定义了所有生态手套（ECO Glove）必须实现的标准接口 `EcoGlove`。
//! 这是 Helix 生态中"原生适配层"的宪法，所有平台手套（macOS/鸿蒙/Linux/机器人）
//! 都必须遵循此接口，确保 Tentacle 可以统一加载和调度。
//!
//! # 设计哲学
//!
//! - **极致解耦**：手套只负责"执行"，不负责"决策"。Tentacle 负责调度，Anaphase 负责编排。
//! - **极致复用**：所有手套共享同一套接口，Tentacle 只需实现一次加载器。
//! - **物理事实优先**：平台标识、宿主 OS、能力声明都是物理事实，不可伪造。
//! - **确定性优先**：工具定义、参数 schema、风险等级都是固定的，执行结果可预测。
//! - **碳基兼容**：所有工具都可以通过 CLI --help 查看，人类可以直接调用。

#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

// ============================================================================
// 基础类型定义
// ============================================================================

/// 平台标识（物理事实，不可伪造）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// macOS
    MacOS,
    /// iOS
    IOS,
    /// 鸿蒙（HarmonyOS）
    Harmony,
    /// Linux
    Linux,
    /// Windows
    Windows,
    /// Android
    Android,
    /// ROS 2 机器人
    Robot,
    /// 通用（不绑定特定平台）
    Generic,
}

impl Platform {
    /// 获取平台的字符串标识
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::MacOS => "macos",
            Platform::IOS => "ios",
            Platform::Harmony => "harmony",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
            Platform::Android => "android",
            Platform::Robot => "robot",
            Platform::Generic => "generic",
        }
    }

    /// 从字符串解析平台标识
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "macos" | "mac" | "darwin" => Some(Platform::MacOS),
            "ios" => Some(Platform::IOS),
            "harmony" | "harmonyos" => Some(Platform::Harmony),
            "linux" => Some(Platform::Linux),
            "windows" | "win" => Some(Platform::Windows),
            "android" => Some(Platform::Android),
            "robot" | "ros" | "ros2" => Some(Platform::Robot),
            "generic" | "universal" => Some(Platform::Generic),
            _ => None,
        }
    }

    /// 获取当前运行平台
    pub fn current() -> Self {
        match std::env::consts::OS {
            "macos" => Platform::MacOS,
            "ios" => Platform::IOS,
            "linux" => Platform::Linux,
            "windows" => Platform::Windows,
            "android" => Platform::Android,
            _ => Platform::Generic,
        }
    }
}

/// 风险等级（与 CI-144 PFP 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    /// 低风险（只读操作，如读取文件、列出目录）
    Low,
    /// 中风险（写操作，如创建文件、更新配置）
    Medium,
    /// 高风险（可能影响系统稳定性，如执行命令、删除文件）
    Critical,
    /// 灾难风险（可能造成不可逆损害，如系统更新、硬件操作）
    Catastrophic,
}

impl RiskLevel {
    /// 获取风险等级的字符串标识
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MEDIUM",
            RiskLevel::Critical => "CRITICAL",
            RiskLevel::Catastrophic => "CATASTROPHIC",
        }
    }

    /// 是否需要人工确认
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, RiskLevel::Critical | RiskLevel::Catastrophic)
    }
}

/// 能力域（用于工具分组和权限控制）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDomain {
    /// 文件系统
    FileSystem,
    /// 进程管理
    Process,
    /// 网络
    Network,
    /// 系统信息
    SystemInfo,
    /// 窗口管理
    Window,
    /// 通知
    Notification,
    /// 传感器
    Sensor,
    /// 硬件控制
    Hardware,
    /// 脚本执行
    Script,
    /// 通用能力域
    Generic,
    /// 自定义能力域
    Custom(String),
}

impl CapabilityDomain {
    /// 获取能力域的字符串标识
    pub fn as_str(&self) -> &str {
        match self {
            CapabilityDomain::FileSystem => "file_system",
            CapabilityDomain::Process => "process",
            CapabilityDomain::Network => "network",
            CapabilityDomain::SystemInfo => "system_info",
            CapabilityDomain::Window => "window",
            CapabilityDomain::Notification => "notification",
            CapabilityDomain::Sensor => "sensor",
            CapabilityDomain::Hardware => "hardware",
            CapabilityDomain::Script => "script",
            CapabilityDomain::Generic => "generic",
            CapabilityDomain::Custom(s) => s.as_str(),
        }
    }
}

// ============================================================================
// 工具定义
// ============================================================================

/// 参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// 参数名
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 参数类型（JSON Schema 类型：string/number/integer/boolean/array/object）
    pub param_type: String,
    /// 是否必填
    pub required: bool,
    /// 默认值（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// 枚举值（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名（点分命名空间：<platform>.<domain>.<action>，如 macos.fs.read_file）
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 能力域
    pub domain: CapabilityDomain,
    /// 风险等级
    pub risk_level: RiskLevel,
    /// 参数列表
    pub parameters: Vec<ToolParameter>,
    /// 返回值描述
    pub return_description: String,
    /// 是否支持 dry_run（无副作用预执行）
    pub supports_dry_run: bool,
    /// 工具版本（语义化版本）
    pub version: String,
    /// 额外元数据（扩展字段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ToolDefinition {
    /// 创建新的工具定义
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        domain: CapabilityDomain,
        risk_level: RiskLevel,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            domain,
            risk_level,
            parameters: Vec::new(),
            return_description: String::new(),
            supports_dry_run: false,
            version: "1.0.0".to_string(),
            metadata: HashMap::new(),
        }
    }

    /// 添加参数
    pub fn with_parameter(mut self, param: ToolParameter) -> Self {
        self.parameters.push(param);
        self
    }

    /// 设置返回值描述
    pub fn with_return_description(mut self, desc: impl Into<String>) -> Self {
        self.return_description = desc.into();
        self
    }

    /// 设置是否支持 dry_run
    pub fn with_dry_run(mut self, supports: bool) -> Self {
        self.supports_dry_run = supports;
        self
    }

    /// 转换为 JSON Schema 格式的参数定义
    pub fn parameters_json_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), serde_json::Value::String(param.param_type.clone()));
            prop.insert("description".to_string(), serde_json::Value::String(param.description.clone()));

            if let Some(default) = &param.default {
                prop.insert("default".to_string(), default.clone());
            }
            if let Some(enum_values) = &param.enum_values {
                prop.insert("enum".to_string(), serde_json::Value::Array(
                    enum_values.iter().map(|s| serde_json::Value::String(s.clone())).collect()
                ));
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(param.name.clone());
            }
        }

        let mut schema = serde_json::Map::new();
        schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        schema.insert("properties".to_string(), serde_json::Value::Object(properties));
        if !required.is_empty() {
            schema.insert("required".to_string(), serde_json::Value::Array(
                required.iter().map(|s| serde_json::Value::String(s.clone())).collect()
            ));
        }

        serde_json::Value::Object(schema)
    }
}

// ============================================================================
// 执行结果
// ============================================================================

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 是否成功
    pub success: bool,
    /// 执行结果数据（成功时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// 错误信息（失败时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 错误码（失败时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 是否为 dry_run 结果
    pub is_dry_run: bool,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
            duration_ms: 0,
            is_dry_run: false,
        }
    }

    /// 创建失败结果
    pub fn failure(error: impl Into<String>, error_code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
            error_code: Some(error_code.into()),
            duration_ms: 0,
            is_dry_run: false,
        }
    }

    /// 设置执行耗时
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// 标记为 dry_run 结果
    pub fn as_dry_run(mut self) -> Self {
        self.is_dry_run = true;
        self
    }
}

// ============================================================================
// 错误类型
// ============================================================================

/// Glove 错误类型
#[derive(Debug, thiserror::Error)]
pub enum GloveError {
    /// 工具不存在
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// 参数验证失败
    #[error("Parameter validation failed: {0}")]
    ParameterValidationFailed(String),

    /// 缺少必填参数
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// 参数类型错误
    #[error("Parameter type error: {0}")]
    ParameterTypeError(String),

    /// 平台不支持
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    /// 权限不足
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// 执行失败
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// 初始化失败
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    /// 未初始化
    #[error("Glove not initialized")]
    NotInitialized,

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

// ============================================================================
// EcoGlove trait 定义
// ============================================================================

/// 异步执行函数类型
pub type AsyncToolExecutor = Box<
    dyn Fn(
            &str,
            serde_json::Value,
            bool,
        ) -> Pin<Box<dyn Future<Output = Result<ToolResult, GloveError>> + Send>>
        + Send
        + Sync,
>;

/// EcoGlove 标准接口
///
/// 所有生态手套必须实现此 trait。这是 Helix 生态中"原生适配层"的宪法。
///
/// # 设计原则
///
/// - **元信息与执行分离**：`platform()`、`host_os()`、`tools()` 是元信息，可静态获取；
///   `execute()` 是执行，需要运行时上下文。
/// - **生命周期管理**：`initialize()` 和 `shutdown()` 管理资源生命周期。
/// - **能力声明**：`capabilities()` 声明手套支持的能力域，用于权限控制和路由。
/// - **平台门控**：`host_os()` 声明支持的宿主 OS，Tentacle 加载时做白名单过滤。
///
/// # 示例
///
/// ```rust
/// use helix_eco_glove_core::*;
/// use std::pin::Pin;
/// use std::future::Future;
///
/// pub struct MacOSGlove;
///
/// impl EcoGlove for MacOSGlove {
///     fn platform(&self) -> Platform { Platform::MacOS }
///     fn host_os(&self) -> Vec<&'static str> { vec!["macos"] }
///     fn version(&self) -> &'static str { "0.1.0" }
///     fn name(&self) -> &'static str { "macos-glove" }
///
///     fn tools(&self) -> Vec<ToolDefinition> {
///         vec![
///             ToolDefinition::new(
///                 "macos.fs.read_file",
///                 "Read a file from the filesystem",
///                 CapabilityDomain::FileSystem,
///                 RiskLevel::Low,
///             )
///             .with_parameter(ToolParameter {
///                 name: "path".to_string(),
///                 description: "File path to read".to_string(),
///                 param_type: "string".to_string(),
///                 required: true,
///                 default: None,
///                 enum_values: None,
///             })
///             .with_return_description("File content as string")
///             .with_dry_run(true),
///         ]
///     }
///
///     fn execute<'a>(&'a self, tool: &'a str, params: serde_json::Value, dry_run: bool)
///         -> Pin<Box<dyn Future<Output = Result<ToolResult, GloveError>> + Send + 'a>> {
///         Box::pin(async move {
///             // 执行逻辑
///             Ok(ToolResult::success(serde_json::json!({"content": "hello"})))
///         })
///     }
/// }
/// ```
pub trait EcoGlove: Send + Sync {
    /// 平台标识（物理事实，不可伪造）
    fn platform(&self) -> Platform;

    /// 支持的宿主 OS 列表（用于 Tentacle 加载时的白名单过滤）
    ///
    /// 返回值示例：`vec!["macos", "ios"]` 表示仅在 macOS 和 iOS 上注册。
    fn host_os(&self) -> Vec<&'static str>;

    /// 手套版本（语义化版本）
    fn version(&self) -> &'static str;

    /// 手套名称（如 "macos-glove"）
    fn name(&self) -> &'static str;

    /// 手套描述
    fn description(&self) -> &'static str {
        ""
    }

    /// 工具定义列表
    fn tools(&self) -> Vec<ToolDefinition>;

    /// 能力声明（支持的能力域列表）
    ///
    /// 默认实现：从工具定义中提取所有能力域。
    fn capabilities(&self) -> Vec<CapabilityDomain> {
        let mut domains: Vec<CapabilityDomain> = self
            .tools()
            .into_iter()
            .map(|t| t.domain)
            .collect();
        domains.dedup();
        domains
    }

    /// 检查当前平台是否支持此手套
    fn is_supported_on_current_platform(&self) -> bool {
        let current = Platform::current().as_str();
        self.host_os().iter().any(|os| *os == current)
    }

    /// 根据工具名查找工具定义
    fn get_tool(&self, name: &str) -> Option<ToolDefinition> {
        self.tools().into_iter().find(|t| t.name == name)
    }

    /// 验证工具参数
    ///
    /// 默认实现：检查必填参数是否存在，参数类型是否匹配。
    fn validate_parameters(&self, tool: &str, params: &serde_json::Value) -> Result<(), GloveError> {
        let tool_def = self
            .get_tool(tool)
            .ok_or_else(|| GloveError::ToolNotFound(tool.to_string()))?;

        for param in &tool_def.parameters {
            let value = params.get(&param.name);

            if param.required && value.is_none() {
                return Err(GloveError::MissingParameter(param.name.clone()));
            }

            if let Some(value) = value {
                let type_valid = match param.param_type.as_str() {
                    "string" => value.is_string(),
                    "number" => value.is_number(),
                    "integer" => value.is_i64() || value.is_u64(),
                    "boolean" => value.is_boolean(),
                    "array" => value.is_array(),
                    "object" => value.is_object(),
                    _ => true, // 未知类型，跳过验证
                };

                if !type_valid {
                    return Err(GloveError::ParameterTypeError(format!(
                        "Parameter '{}' expected type '{}', got '{}'",
                        param.name,
                        param.param_type,
                        value_type_name(value)
                    )));
                }
            }
        }

        Ok(())
    }

    /// 初始化手套（加载配置、建立连接等）
    ///
    /// 默认实现：什么都不做。
    fn initialize(&self) -> Result<(), GloveError> {
        Ok(())
    }

    /// 关闭手套（清理资源）
    ///
    /// 默认实现：什么都不做。
    fn shutdown(&self) -> Result<(), GloveError> {
        Ok(())
    }

    /// 执行工具
    ///
    /// # 参数
    ///
    /// - `tool`: 工具名（点分命名空间）
    /// - `params`: 工具参数（JSON 格式）
    /// - `dry_run`: 是否为预执行模式（无副作用）
    ///
    /// # 返回
    ///
    /// 异步执行结果。
    fn execute<'a>(
        &'a self,
        tool: &'a str,
        params: serde_json::Value,
        dry_run: bool,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, GloveError>> + Send + 'a>>;
}

/// 获取 JSON 值的类型名称
fn value_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

// ============================================================================
// 工具宏（简化工具定义）
// ============================================================================

/// 定义参数的宏
#[macro_export]
macro_rules! param {
    ($name:expr, $desc:expr, $type:expr, $required:expr) => {
        $crate::ToolParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            param_type: $type.to_string(),
            required: $required,
            default: None,
            enum_values: None,
        }
    };
    ($name:expr, $desc:expr, $type:expr, $required:expr, default = $default:expr) => {
        $crate::ToolParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            param_type: $type.to_string(),
            required: $required,
            default: Some($default),
            enum_values: None,
        }
    };
    ($name:expr, $desc:expr, $type:expr, $required:expr, enum = [$($enum:expr),+]) => {
        $crate::ToolParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            param_type: $type.to_string(),
            required: $required,
            default: None,
            enum_values: Some(vec![$($enum.to_string()),+]),
        }
    };
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_current() {
        let current = Platform::current();
        // 在测试环境中，至少应该是一个有效的平台
        assert_ne!(current.as_str(), "");
    }

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from_str("macos"), Some(Platform::MacOS));
        assert_eq!(Platform::from_str("darwin"), Some(Platform::MacOS));
        assert_eq!(Platform::from_str("linux"), Some(Platform::Linux));
        assert_eq!(Platform::from_str("windows"), Some(Platform::Windows));
        assert_eq!(Platform::from_str("harmony"), Some(Platform::Harmony));
        assert_eq!(Platform::from_str("unknown"), None);
    }

    #[test]
    fn test_risk_level_requires_confirmation() {
        assert!(!RiskLevel::Low.requires_confirmation());
        assert!(!RiskLevel::Medium.requires_confirmation());
        assert!(RiskLevel::Critical.requires_confirmation());
        assert!(RiskLevel::Catastrophic.requires_confirmation());
    }

    #[test]
    fn test_tool_definition_builder() {
        let tool = ToolDefinition::new(
            "macos.fs.read_file",
            "Read a file",
            CapabilityDomain::FileSystem,
            RiskLevel::Low,
        )
        .with_parameter(param!("path", "File path", "string", true))
        .with_return_description("File content")
        .with_dry_run(true);

        assert_eq!(tool.name, "macos.fs.read_file");
        assert_eq!(tool.parameters.len(), 1);
        assert_eq!(tool.parameters[0].name, "path");
        assert!(tool.supports_dry_run);
    }

    #[test]
    fn test_tool_definition_json_schema() {
        let tool = ToolDefinition::new(
            "test.tool",
            "Test tool",
            CapabilityDomain::Generic,
            RiskLevel::Low,
        )
        .with_parameter(param!("name", "Name", "string", true))
        .with_parameter(param!("count", "Count", "integer", false));

        let schema = tool.parameters_json_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["name"].is_object());
        assert_eq!(schema["required"][0], "name");
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success(serde_json::json!({"key": "value"}));
        assert!(result.success);
        assert_eq!(result.data.unwrap()["key"], "value");
        assert!(!result.is_dry_run);
    }

    #[test]
    fn test_tool_result_failure() {
        let result = ToolResult::failure("Something went wrong", "E_INTERNAL");
        assert!(!result.success);
        assert_eq!(result.error.unwrap(), "Something went wrong");
        assert_eq!(result.error_code.unwrap(), "E_INTERNAL");
    }

    #[test]
    fn test_glove_error_display() {
        let error = GloveError::ToolNotFound("test.tool".to_string());
        assert_eq!(error.to_string(), "Tool not found: test.tool");
    }

    // 测试用的简单 Glove 实现
    struct TestGlove;

    impl EcoGlove for TestGlove {
        fn platform(&self) -> Platform {
            Platform::Generic
        }

        fn host_os(&self) -> Vec<&'static str> {
            vec!["macos", "linux", "windows"]
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }

        fn name(&self) -> &'static str {
            "test-glove"
        }

        fn tools(&self) -> Vec<ToolDefinition> {
            vec![ToolDefinition::new(
                "test.echo",
                "Echo back the input",
                CapabilityDomain::Generic,
                RiskLevel::Low,
            )
            .with_parameter(param!("message", "Message to echo", "string", true))
            .with_return_description("Echoed message")]
        }

        fn execute<'a>(
            &'a self,
            tool: &'a str,
            params: serde_json::Value,
            _dry_run: bool,
        ) -> Pin<Box<dyn Future<Output = Result<ToolResult, GloveError>> + Send + 'a>> {
            Box::pin(async move {
                self.validate_parameters(tool, &params)?;
                let message = params["message"].as_str().unwrap_or("").to_string();
                Ok(ToolResult::success(serde_json::json!({"echo": message})))
            })
        }
    }

    #[tokio::test]
    async fn test_ecoglove_execute() {
        let glove = TestGlove;
        glove.initialize().unwrap();

        let result = glove
            .execute(
                "test.echo",
                serde_json::json!({"message": "hello"}),
                false,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data.unwrap()["echo"], "hello");

        glove.shutdown().unwrap();
    }

    #[tokio::test]
    async fn test_ecoglove_validate_parameters() {
        let glove = TestGlove;

        // 缺少必填参数
        let result = glove
            .execute("test.echo", serde_json::json!({}), false)
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GloveError::MissingParameter(_)
        ));

        // 工具不存在
        let result = glove
            .execute("nonexistent.tool", serde_json::json!({}), false)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GloveError::ToolNotFound(_)));
    }

    #[test]
    fn test_ecoglove_capabilities() {
        let glove = TestGlove;
        let capabilities = glove.capabilities();
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].as_str(), "generic");
    }

    #[test]
    fn test_ecoglove_get_tool() {
        let glove = TestGlove;
        let tool = glove.get_tool("test.echo");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "test.echo");

        assert!(glove.get_tool("nonexistent").is_none());
    }
}
