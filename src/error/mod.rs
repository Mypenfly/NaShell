use std::fmt;

/// NaShell 统一错误类型
#[derive(Debug)]
pub enum NashellError {
    /// 解析错误
    Parse {
        /// 解析上下文（如输入字符串）
        context: String,
        /// 错误详情
        detail: String,
    },
    /// 执行错误
    Execute {
        /// 执行的命令
        command: String,
        /// 退出码（如果可用）
        exit_code: Option<i32>,
        /// 标准错误输出
        stderr: String,
    },
    /// 配置错误
    Config {
        /// 配置文件路径
        path: String,
        /// 错误详情
        detail: String,
    },
    /// IO 错误
    Io {
        /// 相关文件路径
        path: Option<String>,
        /// 底层 IO 错误
        source: std::io::Error,
    },
    /// 插件错误
    Plugin {
        /// 插件名称
        plugin_name: String,
        /// 错误详情
        detail: String,
    },
    /// 命令未找到
    CommandNotFound {
        /// 命令名称
        name: String,
    },
    /// 超时
    Timeout {
        /// 超时的命令
        command: String,
        /// 超时秒数
        seconds: u64,
    },
    /// 安全拦截
    SafetyBlocked {
        /// 被拦截的命令
        command: String,
        /// 拦截原因
        reason: String,
    },
}

impl fmt::Display for NashellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NashellError::Parse { context, detail } => {
                write!(f, "Parse error in '{}': {}", context, detail)
            }
            NashellError::Execute {
                command,
                exit_code,
                stderr,
            } => {
                if let Some(code) = exit_code {
                    write!(
                        f,
                        "Command '{}' failed with exit code {}: {}",
                        command, code, stderr
                    )
                } else {
                    write!(f, "Command '{}' failed: {}", command, stderr)
                }
            }
            NashellError::Config { path, detail } => {
                write!(f, "Config error in '{}': {}", path, detail)
            }
            NashellError::Io { path, source } => {
                if let Some(p) = path {
                    write!(f, "IO error for '{}': {}", p, source)
                } else {
                    write!(f, "IO error: {}", source)
                }
            }
            NashellError::Plugin {
                plugin_name,
                detail,
            } => {
                write!(f, "Plugin '{}' error: {}", plugin_name, detail)
            }
            NashellError::CommandNotFound { name } => {
                write!(f, "Command not found: {}", name)
            }
            NashellError::Timeout { command, seconds } => {
                write!(
                    f,
                    "Command '{}' timed out after {} seconds",
                    command, seconds
                )
            }
            NashellError::SafetyBlocked { command, reason } => {
                write!(f, "Command '{}' blocked by safety: {}", command, reason)
            }
        }
    }
}

impl std::error::Error for NashellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NashellError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_parse_error_display() {
        let err = NashellError::Parse {
            context: "test_input".to_string(),
            detail: "unexpected token".to_string(),
        };
        let display_str = format!("{}", err);
        assert!(display_str.contains("Parse") || display_str.contains("parse"));
        assert!(display_str.contains("test_input"));
        assert!(display_str.contains("unexpected token"));
    }

    #[test]
    fn test_command_not_found() {
        let err = NashellError::CommandNotFound {
            name: "unknown_cmd".to_string(),
        };
        let display_str = format!("{}", err);
        assert!(display_str.contains("unknown_cmd"));
        assert!(display_str.contains("not found"));
    }

    #[test]
    fn test_execute_error_display() {
        let err = NashellError::Execute {
            command: "ls".to_string(),
            exit_code: Some(2),
            stderr: "No such file".to_string(),
        };
        let display_str = format!("{}", err);
        assert!(display_str.contains("ls"));
        assert!(display_str.contains("2"));
        assert!(display_str.contains("No such file"));
    }

    #[test]
    fn test_io_error_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = NashellError::Io {
            path: Some("/tmp/test".to_string()),
            source: io_err,
        };
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_trait() {
        let err = NashellError::CommandNotFound {
            name: "test".to_string(),
        };
        let _: &dyn std::error::Error = &err;
    }
}
