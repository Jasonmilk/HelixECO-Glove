//! Helix ECO Glove → Helix-Tentacle 适配器
//!
//! 将实现了 `EcoGlove` trait 的手套转换为 Tentacle 的 `Manifest` + `Tool`，
//! 使 Tentacle 可以统一加载和执行所有平台手套。
//!
//! # 设计哲学
//!
//! - **极致解耦**：Tentacle 不需要知道 EcoGlove 的存在，适配器负责转换
//! - **极致复用**：所有手套共享同一套转换逻辑，Tentacle 只需实现一次加载器
//! - **平台感知**：转换时保留平台信息，Tentacle 可以做平台白名单过滤
//! - **物理事实优先**：风险等级、平台标识都是物理事实，转换时不改变

#![deny(missing_docs)]

use helix_eco_glove_core::*;
use std::sync::Arc;
use tentacle_core::manifest::{Integrity, Manifest, PlatformSupport, SecurityLevel};
use tentacle_core::tool::{ExecutionRequest, Tool, ToolOutput};
use tentacle_core::ToolRegistry;

// ============================================================================
// 错误类型
// ============================================================================

/// 适配器错误
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// 工具不存在
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    /// 手套执行错误
    #[error("Glove execution error: {0}")]
    GloveError(String),
    /// 注册错误
    #[error("Registry error: {0}")]
    RegistryError(String),
}

impl From<GloveError> for AdapterError {
    fn from(e: GloveError) -> Self {
        AdapterError::GloveError(e.to_string())
    }
}

impl From<tentacle_core::RegistryError> for AdapterError {
    fn from(e: tentacle_core::RegistryError) -> Self {
        AdapterError::RegistryError(e.to_string())
    }
}

// ============================================================================
// 类型转换
// ============================================================================

/// 将 EcoGlove 的 RiskLevel 转换为 Tentacle 的 SecurityLevel
fn risk_to_security(risk: &RiskLevel) -> SecurityLevel {
    match risk {
        RiskLevel::Low | RiskLevel::Medium => SecurityLevel::Normal,
        RiskLevel::Critical | RiskLevel::Catastrophic => SecurityLevel::Critical,
    }
}

/// 将 EcoGlove 的 ToolDefinition 转换为 Tentacle 的 Manifest
fn tool_definition_to_manifest(
    tool: &ToolDefinition,
    glove: &dyn EcoGlove,
) -> Manifest {
    Manifest {
        name: tool.name.clone(),
        version: tool.version.clone(),
        description: tool.description.clone(),
        author: Some(glove.name().to_string()),
        tags: vec![
            glove.platform().as_str().to_string(),
            tool.domain.as_str().to_string(),
            tool.risk_level.as_str().to_string(),
        ],
        executable: format!("glove://{}/{}", glove.name(), tool.name),
        integrity: Integrity::default(),
        parameters_schema: tool.parameters_json_schema(),
        examples: Vec::new(),
        trigger_phrases: Vec::new(),
        security_level: risk_to_security(&tool.risk_level),
        permissions: Default::default(),
        requires_identity: None,
        requires_external_signer: false,
        allowed_signing_domains: Vec::new(),
        rate_limit_per_minute: None,
        timeout_ms: 30000,
        foraging_config: Default::default(),
        platform_support: PlatformSupport::new(
            glove.platform().as_str(),
            glove.host_os().iter().map(|s| s.to_string()).collect(),
        ),
    }
}

// ============================================================================
// GloveTool — 实现 Tentacle Tool trait 的包装器
// ============================================================================

/// GloveTool — 将 EcoGlove 的工具包装为 Tentacle 的 Tool
///
/// 持有 Arc<dyn EcoGlove> 和工具名，执行时转发给手套。
pub struct GloveTool {
    glove: Arc<dyn EcoGlove>,
    manifest: Manifest,
}

impl GloveTool {
    /// 创建新的 GloveTool
    pub fn new(glove: Arc<dyn EcoGlove>, tool_name: &str) -> Result<Self, AdapterError> {
        let tool_def = glove
            .get_tool(tool_name)
            .ok_or_else(|| AdapterError::ToolNotFound(tool_name.to_string()))?;

        let manifest = tool_definition_to_manifest(&tool_def, glove.as_ref());

        Ok(Self { glove, manifest })
    }

    /// 获取手套引用
    pub fn glove(&self) -> &dyn EcoGlove {
        self.glove.as_ref()
    }
}

impl Tool for GloveTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn execute(&self, req: ExecutionRequest) -> Result<ToolOutput, tentacle_core::ToolError> {
        // 执行手套工具（异步转同步）
        let glove_result: Result<ToolResult, GloveError> = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async {
                self.glove.execute(&req.tool, req.params.clone(), false).await
            })
        } else {
            // 没有当前 runtime，创建一个新的
            let runtime = tokio::runtime::Runtime::new().map_err(|e| {
                tentacle_core::ToolError::ExecutionFailed(format!(
                    "Failed to create runtime: {}",
                    e
                ))
            })?;
            runtime.block_on(async {
                self.glove.execute(&req.tool, req.params.clone(), false).await
            })
        };

        match glove_result {
            Ok(tool_result) => {
                if tool_result.success {
                    let mut output = ToolOutput::success(
                        tool_result.data.unwrap_or(serde_json::Value::Null),
                    );
                    output.duration_ms = Some(tool_result.duration_ms);
                    Ok(output)
                } else {
                    let mut output = ToolOutput::failure(
                        tool_result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    );
                    output.duration_ms = Some(tool_result.duration_ms);
                    Ok(output)
                }
            }
            Err(e) => Err(tentacle_core::ToolError::ExecutionFailed(e.to_string())),
        }
    }
}

// ============================================================================
// GloveAdapter — 手套适配器
// ============================================================================

/// GloveAdapter — 将 EcoGlove 实现转换为 Tentacle 工具的适配器
pub struct GloveAdapter {
    glove: Arc<dyn EcoGlove>,
}

impl GloveAdapter {
    /// 创建新的适配器
    pub fn new(glove: Arc<dyn EcoGlove>) -> Self {
        Self { glove }
    }

    /// 获取手套引用
    pub fn glove(&self) -> &dyn EcoGlove {
        self.glove.as_ref()
    }

    /// 获取所有工具的 Manifest 列表
    pub fn manifests(&self) -> Vec<Manifest> {
        self.glove
            .tools()
            .iter()
            .map(|tool| tool_definition_to_manifest(tool, self.glove.as_ref()))
            .collect()
    }

    /// 获取所有工具的 GloveTool 实例
    pub fn tools(&self) -> Result<Vec<Arc<dyn Tool>>, AdapterError> {
        let tool_names: Vec<String> = self.glove.tools().iter().map(|t| t.name.clone()).collect();

        let mut tools = Vec::new();
        for name in &tool_names {
            let tool = GloveTool::new(self.glove.clone(), name)?;
            tools.push(Arc::new(tool) as Arc<dyn Tool>);
        }

        Ok(tools)
    }

    /// 获取指定工具的 GloveTool 实例
    pub fn tool(&self, tool_name: &str) -> Result<Arc<dyn Tool>, AdapterError> {
        let tool = GloveTool::new(self.glove.clone(), tool_name)?;
        Ok(Arc::new(tool) as Arc<dyn Tool>)
    }

    /// 注册所有工具到 ToolRegistry
    pub fn register_all(&self, registry: &mut ToolRegistry) -> Result<(), AdapterError> {
        let tools = self.tools()?;
        for tool in tools {
            registry.register_tool(tool)?;
        }
        Ok(())
    }

    /// 只注册当前平台支持的工具
    pub fn register_supported(&self, registry: &mut ToolRegistry) -> Result<(), AdapterError> {
        if !self.glove.is_supported_on_current_platform() {
            tracing::info!(
                "Skipping glove {}: only supports {:?}",
                self.glove.name(),
                self.glove.host_os()
            );
            return Ok(());
        }
        self.register_all(registry)
    }

    /// 获取 ManifestIndex 列表（用于渐进披露）
    pub fn manifest_indices(&self) -> Vec<tentacle_core::ManifestIndex> {
        self.manifests()
            .iter()
            .map(tentacle_core::ManifestIndex::from)
            .collect()
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 便捷函数：将一个 EcoGlove 实现注册到 ToolRegistry
///
/// # 示例
///
/// ```rust,no_run
/// use helix_eco_glove_core::*;
/// use helix_eco_glove_macos::MacOSGlove;
/// use helix_eco_glove_tentacle_adapter::register_glove;
/// use tentacle_core::ToolRegistry;
/// use std::sync::Arc;
///
/// let mut registry = ToolRegistry::new();
/// let glove = Arc::new(MacOSGlove::new()) as Arc<dyn EcoGlove>;
/// register_glove(&glove, &mut registry).unwrap();
/// ```
pub fn register_glove(
    glove: &Arc<dyn EcoGlove>,
    registry: &mut ToolRegistry,
) -> Result<(), AdapterError> {
    let adapter = GloveAdapter::new(glove.clone());
    adapter.register_supported(registry)
}

/// 便捷函数：将多个 EcoGlove 实现注册到 ToolRegistry
pub fn register_gloves(
    gloves: &[Arc<dyn EcoGlove>],
    registry: &mut ToolRegistry,
) -> Result<(), AdapterError> {
    for glove in gloves {
        register_glove(glove, registry)?;
    }
    Ok(())
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use helix_eco_glove_macos::MacOSGlove;
    use std::collections::HashMap;

    fn test_glove() -> Arc<dyn EcoGlove> {
        Arc::new(MacOSGlove::new()) as Arc<dyn EcoGlove>
    }

    #[test]
    fn test_risk_to_security() {
        assert_eq!(risk_to_security(&RiskLevel::Low), SecurityLevel::Normal);
        assert_eq!(risk_to_security(&RiskLevel::Medium), SecurityLevel::Normal);
        assert_eq!(risk_to_security(&RiskLevel::Critical), SecurityLevel::Critical);
        assert_eq!(risk_to_security(&RiskLevel::Catastrophic), SecurityLevel::Critical);
    }

    #[test]
    fn test_tool_definition_to_manifest() {
        let glove = test_glove();
        let tools = glove.tools();
        let tool = tools.first().unwrap();
        let manifest = tool_definition_to_manifest(tool, glove.as_ref());

        assert_eq!(manifest.name, tool.name);
        assert_eq!(manifest.version, tool.version);
        assert_eq!(manifest.description, tool.description);
        assert!(!manifest.tags.is_empty());
        assert_eq!(
            manifest.platform_support.platform,
            glove.platform().as_str()
        );
    }

    #[test]
    fn test_glove_adapter_manifests() {
        let glove = test_glove();
        let adapter = GloveAdapter::new(glove);

        let manifests = adapter.manifests();
        assert_eq!(manifests.len(), 6); // macOS Glove 有 6 个工具

        // 验证所有 manifest 都有平台支持信息
        for manifest in &manifests {
            assert!(!manifest.platform_support.platform.is_empty());
        }
    }

    #[test]
    fn test_glove_adapter_tools() {
        let glove = test_glove();
        let adapter = GloveAdapter::new(glove);

        let tools = adapter.tools().unwrap();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_glove_adapter_tool() {
        let glove = test_glove();
        let adapter = GloveAdapter::new(glove);

        let tool = adapter.tool("macos.fs.read_file").unwrap();
        assert_eq!(tool.name(), "macos.fs.read_file");

        // 测试不存在的工具
        let result = adapter.tool("nonexistent.tool");
        match result {
            Err(AdapterError::ToolNotFound(_)) => {}
            Err(other) => panic!("Expected ToolNotFound, got other error"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn test_register_glove() {
        let glove = test_glove();
        let mut registry = ToolRegistry::new();

        register_glove(&glove, &mut registry).unwrap();

        // 验证注册了 6 个工具（如果当前平台是 macOS）
        // 如果不是 macOS，register_supported 会跳过，所以这里只检查不报错
    }

    #[test]
    fn test_glove_tool_execute() {
        let glove = test_glove();
        let adapter = GloveAdapter::new(glove);
        let tool = adapter.tool("macos.fs.list_directory").unwrap();

        // 创建临时目录
        let temp_dir = tempfile::tempdir().unwrap();

        let req = ExecutionRequest {
            tool: "macos.fs.list_directory".to_string(),
            params: serde_json::json!({"directory": temp_dir.path().to_string_lossy()}),
            identity_labels: HashMap::new(),
            trace_id: None,
            seen_entropy_bloom: None,
        };

        let result = tool.execute(req).unwrap();
        assert!(result.ok);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_manifest_indices() {
        let glove = test_glove();
        let adapter = GloveAdapter::new(glove);

        let indices = adapter.manifest_indices();
        assert_eq!(indices.len(), 6);

        for index in &indices {
            assert!(!index.name.is_empty());
            assert!(!index.platform.is_empty());
        }
    }
}
