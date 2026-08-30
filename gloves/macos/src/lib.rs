//! Helix ECO Glove for macOS
//!
//! 原生 macOS 系统工具适配，实现 EcoGlove trait。
//! 提供文件系统、进程管理、AppleScript 执行等 6 个工具。

#![deny(missing_docs)]

use helix_eco_glove_core::*;
use std::future::Future;
use std::pin::Pin;
use std::process::Command;

// ============================================================================
// macOS Glove 实现
// ============================================================================

/// macOS Glove
pub struct MacOSGlove;

impl MacOSGlove {
    /// 创建新的 macOS Glove 实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSGlove {
    fn default() -> Self {
        Self::new()
    }
}

impl EcoGlove for MacOSGlove {
    fn platform(&self) -> Platform {
        Platform::MacOS
    }

    fn host_os(&self) -> Vec<&'static str> {
        vec!["macos"]
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn name(&self) -> &'static str {
        "macos-glove"
    }

    fn description(&self) -> &'static str {
        "Native macOS system tools: filesystem, process, AppleScript"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![
            // 文件系统工具
            ToolDefinition::new(
                "macos.fs.read_file",
                "Read a file from the macOS filesystem",
                CapabilityDomain::FileSystem,
                RiskLevel::Low,
            )
            .with_parameter(param!("path", "File path to read", "string", true))
            .with_return_description("File content as string")
            .with_dry_run(true),

            ToolDefinition::new(
                "macos.fs.write_file",
                "Write content to a file on the macOS filesystem",
                CapabilityDomain::FileSystem,
                RiskLevel::Medium,
            )
            .with_parameter(param!("path", "File path to write", "string", true))
            .with_parameter(param!("content", "Content to write", "string", true))
            .with_return_description("Write result confirmation")
            .with_dry_run(true),

            ToolDefinition::new(
                "macos.fs.list_directory",
                "List files in a directory on macOS",
                CapabilityDomain::FileSystem,
                RiskLevel::Low,
            )
            .with_parameter(param!("directory", "Directory path to list", "string", true))
            .with_return_description("List of file names in the directory")
            .with_dry_run(true),

            // 进程管理工具
            ToolDefinition::new(
                "macos.process.execute",
                "Execute a shell command on macOS",
                CapabilityDomain::Process,
                RiskLevel::Critical,
            )
            .with_parameter(param!("command", "Command to execute", "string", true))
            .with_parameter(param!("args", "Command arguments", "array", false))
            .with_return_description("Command output (stdout + stderr) and exit code")
            .with_dry_run(false),

            ToolDefinition::new(
                "macos.process.list",
                "List running processes on macOS",
                CapabilityDomain::Process,
                RiskLevel::Low,
            )
            .with_return_description("List of running processes (PID + command name)")
            .with_dry_run(true),

            // 脚本执行工具
            ToolDefinition::new(
                "macos.script.applescript",
                "Execute an AppleScript on macOS",
                CapabilityDomain::Script,
                RiskLevel::Critical,
            )
            .with_parameter(param!("script", "AppleScript code to execute", "string", true))
            .with_return_description("AppleScript execution result")
            .with_dry_run(false),
        ]
    }

    fn execute<'a>(
        &'a self,
        tool: &'a str,
        params: serde_json::Value,
        dry_run: bool,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, GloveError>> + Send + 'a>> {
        Box::pin(async move {
            let start = std::time::Instant::now();

            // 验证参数
            self.validate_parameters(tool, &params)?;

            let result = match tool {
                "macos.fs.read_file" => execute_read_file(&params, dry_run),
                "macos.fs.write_file" => execute_write_file(&params, dry_run),
                "macos.fs.list_directory" => execute_list_directory(&params, dry_run),
                "macos.process.execute" => execute_command(&params, dry_run),
                "macos.process.list" => execute_list_processes(dry_run),
                "macos.script.applescript" => execute_applescript(&params, dry_run),
                _ => return Err(GloveError::ToolNotFound(tool.to_string())),
            }?;

            let duration = start.elapsed().as_millis() as u64;
            Ok(result.with_duration(duration))
        })
    }
}

// ============================================================================
// 工具执行实现
// ============================================================================

/// 执行 read_file
fn execute_read_file(params: &serde_json::Value, dry_run: bool) -> Result<ToolResult, GloveError> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("path".to_string()))?;

    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "read_file",
            "path": path,
            "message": "Would read file"
        }))
        .as_dry_run());
    }

    match std::fs::read_to_string(path) {
        Ok(content) => Ok(ToolResult::success(serde_json::json!({
            "path": path,
            "content": content,
            "size": content.len()
        }))),
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to read file '{}': {}",
            path, e
        ))),
    }
}

/// 执行 write_file
fn execute_write_file(params: &serde_json::Value, dry_run: bool) -> Result<ToolResult, GloveError> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("path".to_string()))?;
    let content = params["content"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("content".to_string()))?;

    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "write_file",
            "path": path,
            "content_size": content.len(),
            "message": "Would write file"
        }))
        .as_dry_run());
    }

    match std::fs::write(path, content) {
        Ok(_) => Ok(ToolResult::success(serde_json::json!({
            "path": path,
            "bytes_written": content.len(),
            "message": "File written successfully"
        }))),
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to write file '{}': {}",
            path, e
        ))),
    }
}

/// 执行 list_directory
fn execute_list_directory(params: &serde_json::Value, dry_run: bool) -> Result<ToolResult, GloveError> {
    let directory = params["directory"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("directory".to_string()))?;

    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "list_directory",
            "directory": directory,
            "message": "Would list directory"
        }))
        .as_dry_run());
    }

    match std::fs::read_dir(directory) {
        Ok(entries) => {
            let files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_dir = e.path().is_dir();
                    format!("{}{}", name, if is_dir { "/" } else { "" })
                })
                .collect();

            Ok(ToolResult::success(serde_json::json!({
                "directory": directory,
                "files": files,
                "count": files.len()
            })))
        }
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to list directory '{}': {}",
            directory, e
        ))),
    }
}

/// 执行 command
fn execute_command(params: &serde_json::Value, dry_run: bool) -> Result<ToolResult, GloveError> {
    let command = params["command"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("command".to_string()))?;

    let args: Vec<String> = params["args"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "execute_command",
            "command": command,
            "args": args,
            "message": "Would execute command (dry_run not supported for process.execute)"
        }))
        .as_dry_run());
    }

    match Command::new(command).args(&args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            if output.status.success() {
                Ok(ToolResult::success(serde_json::json!({
                    "command": command,
                    "args": args,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code
                })))
            } else {
                Ok(ToolResult::failure(
                    format!(
                        "Command failed with exit code {}: {}",
                        exit_code,
                        if stderr.is_empty() { stdout } else { stderr }
                    ),
                    "E_COMMAND_FAILED",
                ))
            }
        }
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to execute command '{}': {}",
            command, e
        ))),
    }
}

/// 执行 list_processes
fn execute_list_processes(dry_run: bool) -> Result<ToolResult, GloveError> {
    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "list_processes",
            "message": "Would list running processes"
        }))
        .as_dry_run());
    }

    match Command::new("ps").args(["-axo", "pid,comm"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let processes: Vec<serde_json::Value> = stdout
                .lines()
                .skip(1) // 跳过表头
                .filter_map(|line| {
                    let parts: Vec<&str> = line.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let pid = parts[0].trim().parse::<i32>().ok()?;
                        let comm = parts[1].trim().to_string();
                        Some(serde_json::json!({
                            "pid": pid,
                            "command": comm
                        }))
                    } else {
                        None
                    }
                })
                .collect();

            Ok(ToolResult::success(serde_json::json!({
                "processes": processes,
                "count": processes.len()
            })))
        }
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to list processes: {}",
            e
        ))),
    }
}

/// 执行 applescript
fn execute_applescript(params: &serde_json::Value, dry_run: bool) -> Result<ToolResult, GloveError> {
    let script = params["script"]
        .as_str()
        .ok_or_else(|| GloveError::MissingParameter("script".to_string()))?;

    if dry_run {
        return Ok(ToolResult::success(serde_json::json!({
            "dry_run": true,
            "action": "run_applescript",
            "script_length": script.len(),
            "message": "Would execute AppleScript (dry_run not supported for script.applescript)"
        }))
        .as_dry_run());
    }

    match Command::new("osascript").args(["-e", script]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                Ok(ToolResult::success(serde_json::json!({
                    "result": stdout,
                    "stderr": stderr
                })))
            } else {
                Ok(ToolResult::failure(
                    if stderr.is_empty() {
                        "AppleScript execution failed".to_string()
                    } else {
                        stderr
                    },
                    "E_APPLESCRIPT_FAILED",
                ))
            }
        }
        Err(e) => Err(GloveError::ExecutionFailed(format!(
            "Failed to execute AppleScript: {}",
            e
        ))),
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_glove_metadata() {
        let glove = MacOSGlove::new();
        assert_eq!(glove.platform(), Platform::MacOS);
        assert_eq!(glove.host_os(), vec!["macos"]);
        assert_eq!(glove.name(), "macos-glove");
        assert!(!glove.version().is_empty());
    }

    #[test]
    fn test_macos_glove_tools_count() {
        let glove = MacOSGlove::new();
        let tools = glove.tools();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_macos_glove_tool_names() {
        let glove = MacOSGlove::new();
        let tools = glove.tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(names.contains(&"macos.fs.read_file"));
        assert!(names.contains(&"macos.fs.write_file"));
        assert!(names.contains(&"macos.fs.list_directory"));
        assert!(names.contains(&"macos.process.execute"));
        assert!(names.contains(&"macos.process.list"));
        assert!(names.contains(&"macos.script.applescript"));
    }

    #[test]
    fn test_macos_glove_risk_levels() {
        let glove = MacOSGlove::new();
        let tools = glove.tools();

        // 只读操作应该是 LOW
        let read_file = tools.iter().find(|t| t.name == "macos.fs.read_file").unwrap();
        assert_eq!(read_file.risk_level, RiskLevel::Low);

        // 写操作应该是 MEDIUM
        let write_file = tools.iter().find(|t| t.name == "macos.fs.write_file").unwrap();
        assert_eq!(write_file.risk_level, RiskLevel::Medium);

        // 命令执行应该是 CRITICAL
        let execute = tools.iter().find(|t| t.name == "macos.process.execute").unwrap();
        assert_eq!(execute.risk_level, RiskLevel::Critical);
        assert!(execute.risk_level.requires_confirmation());
    }

    #[test]
    fn test_macos_glove_capabilities() {
        let glove = MacOSGlove::new();
        let capabilities = glove.capabilities();

        assert!(capabilities.iter().any(|c| c.as_str() == "file_system"));
        assert!(capabilities.iter().any(|c| c.as_str() == "process"));
        assert!(capabilities.iter().any(|c| c.as_str() == "script"));
    }

    #[tokio::test]
    async fn test_read_file_dry_run() {
        let glove = MacOSGlove::new();
        let result = glove
            .execute(
                "macos.fs.read_file",
                serde_json::json!({"path": "/tmp/nonexistent.txt"}),
                true,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.is_dry_run);
        assert_eq!(result.data.unwrap()["action"], "read_file");
    }

    #[tokio::test]
    async fn test_read_write_file() {
        let glove = MacOSGlove::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // 写入文件
        let write_result = glove
            .execute(
                "macos.fs.write_file",
                serde_json::json!({
                    "path": test_file.to_string_lossy(),
                    "content": "Hello macOS Glove!"
                }),
                false,
            )
            .await
            .unwrap();

        assert!(write_result.success);
        assert_eq!(write_result.data.unwrap()["bytes_written"], 18);

        // 读取文件
        let read_result = glove
            .execute(
                "macos.fs.read_file",
                serde_json::json!({"path": test_file.to_string_lossy()}),
                false,
            )
            .await
            .unwrap();

        assert!(read_result.success);
        assert_eq!(read_result.data.unwrap()["content"], "Hello macOS Glove!");
    }

    #[tokio::test]
    async fn test_list_directory() {
        let glove = MacOSGlove::new();
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "test").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "test").unwrap();

        let result = glove
            .execute(
                "macos.fs.list_directory",
                serde_json::json!({"directory": temp_dir.path().to_string_lossy()}),
                false,
            )
            .await
            .unwrap();

        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["count"], 2);
        let files: Vec<String> = data["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert!(files.contains(&"file1.txt".to_string()));
        assert!(files.contains(&"file2.txt".to_string()));
    }

    #[tokio::test]
    async fn test_execute_command() {
        let glove = MacOSGlove::new();
        let result = glove
            .execute(
                "macos.process.execute",
                serde_json::json!({
                    "command": "echo",
                    "args": ["Hello World"]
                }),
                false,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.data.unwrap()["stdout"].as_str().unwrap().contains("Hello World"));
    }

    #[tokio::test]
    async fn test_list_processes() {
        let glove = MacOSGlove::new();
        let result = glove
            .execute("macos.process.list", serde_json::json!({}), false)
            .await
            .unwrap();

        assert!(result.success);
        let data = result.data.unwrap();
        assert!(data["count"].as_i64().unwrap() > 0);
        assert!(data["processes"].is_array());
    }

    #[tokio::test]
    async fn test_tool_not_found() {
        let glove = MacOSGlove::new();
        let result = glove
            .execute("nonexistent.tool", serde_json::json!({}), false)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GloveError::ToolNotFound(_)));
    }

    #[tokio::test]
    async fn test_missing_parameter() {
        let glove = MacOSGlove::new();
        let result = glove
            .execute("macos.fs.read_file", serde_json::json!({}), false)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GloveError::MissingParameter(_)));
    }

    #[test]
    fn test_is_supported_on_current_platform() {
        let glove = MacOSGlove::new();
        // 在 macOS 上应该返回 true，在其他平台上返回 false
        let current = Platform::current();
        if current == Platform::MacOS {
            assert!(glove.is_supported_on_current_platform());
        } else {
            assert!(!glove.is_supported_on_current_platform());
        }
    }
}
