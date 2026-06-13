use crate::error::NashellError;

/// 将 NashellError 格式化为 NaShell 统一错误输出格式。
///
/// 格式：`@Error #>>\n{类型}: {描述}`
///
/// 每种错误类型有对应的类型名称标签：
/// - Parse → "解析错误"
/// - Execute → "执行错误"
/// - Config → "配置错误"
/// - Io → "IO 错误"
/// - Plugin → "插件错误"
/// - CommandNotFound → "命令未找到"
/// - Timeout → "执行超时"
/// - SafetyBlocked → "安全拦截"
///
/// # 参数
/// - `err`: NaShell 统一错误类型
///
/// # 返回
/// 格式化后的错误字符串
///
/// # 示例
/// ```
/// use nashell::error::NashellError;
/// let err = NashellError::CommandNotFound { name: "unknown".into() };
/// let formatted = nashell::error::display::format_error(&err);
/// assert!(formatted.starts_with("@Error #>>"));
/// ```
pub fn format_error(err: &NashellError) -> String {
    let (err_type, description) = match err {
        NashellError::Parse { context, detail } => {
            ("解析错误", format!("在 '{}': {}", context, detail))
        }
        NashellError::Execute {
            command,
            exit_code,
            stderr,
        } => {
            if let Some(code) = exit_code {
                (
                    "执行错误",
                    format!("命令 '{}' 退出码 {}: {}", command, code, stderr),
                )
            } else {
                ("执行错误", format!("命令 '{}' 失败: {}", command, stderr))
            }
        }
        NashellError::Config { path, detail } => {
            ("配置错误", format!("文件 '{}': {}", path, detail))
        }
        NashellError::Io { path, source } => {
            if let Some(p) = path {
                ("IO 错误", format!("路径 '{}': {}", p, source))
            } else {
                ("IO 错误", format!("{}", source))
            }
        }
        NashellError::Plugin {
            plugin_name,
            detail,
        } => {
            ("插件错误", format!("插件 '{}': {}", plugin_name, detail))
        }
        NashellError::CommandNotFound { name } => {
            ("命令未找到", format!("找不到命令 '{}'", name))
        }
        NashellError::Timeout { command, seconds } => {
            (
                "执行超时",
                format!("命令 '{}' 超时 ({} 秒)", command, seconds),
            )
        }
        NashellError::SafetyBlocked { command, reason } => {
            ("安全拦截", format!("命令 '{}' 被拦截: {}", command, reason))
        }
    };

    format!("@Error #>>\n{}: {}", err_type, description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NashellError;

    #[test]
    fn test_format_parse_error() {
        let err = NashellError::Parse {
            context: "ls | !@".into(),
            detail: "unexpected token".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n解析错误:"));
        assert!(formatted.contains("ls | !@"));
        assert!(formatted.contains("unexpected token"));
    }

    #[test]
    fn test_format_execute_error() {
        let err = NashellError::Execute {
            command: "ls".into(),
            exit_code: Some(2),
            stderr: "No such file".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n执行错误:"));
        assert!(formatted.contains("ls"));
        assert!(formatted.contains("2"));
        assert!(formatted.contains("No such file"));
    }

    #[test]
    fn test_format_config_error() {
        let err = NashellError::Config {
            path: "/tmp/config.kdl".into(),
            detail: "invalid syntax".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n配置错误:"));
        assert!(formatted.contains("/tmp/config.kdl"));
    }

    #[test]
    fn test_format_command_not_found() {
        let err = NashellError::CommandNotFound {
            name: "unknown_cmd".into(),
        };
        let formatted = format_error(&err);
        assert_eq!(
            formatted,
            "@Error #>>\n命令未找到: 找不到命令 'unknown_cmd'"
        );
    }

    #[test]
    fn test_format_timeout() {
        let err = NashellError::Timeout {
            command: "sleep".into(),
            seconds: 30,
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n执行超时:"));
        assert!(formatted.contains("sleep"));
        assert!(formatted.contains("30"));
    }

    #[test]
    fn test_format_safety_blocked() {
        let err = NashellError::SafetyBlocked {
            command: "rm -rf /".into(),
            reason: "deny_pattern matched".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n安全拦截:"));
        assert!(formatted.contains("rm -rf /"));
    }

    #[test]
    fn test_format_plugin_error() {
        let err = NashellError::Plugin {
            plugin_name: "agent".into(),
            detail: "connection lost".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n插件错误:"));
        assert!(formatted.contains("agent"));
    }

    #[test]
    fn test_format_io_error() {
        let err = NashellError::Io {
            path: Some("/tmp/test".into()),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\nIO 错误:"));
        assert!(formatted.contains("/tmp/test"));
    }
}
