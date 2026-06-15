use crate::error::NashellError;

/// ANSI 绿色前景色转义码（用于 Hint 提示）。
const GREEN: &str = "\x1b[32m";
/// ANSI 重置码。
const RESET: &str = "\x1b[0m";

/// 为文本包裹绿色前景色。
fn green(text: &str) -> String {
    format!("{}{}{}", GREEN, text, RESET)
}

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
/// let err = NashellError::CommandNotFound { name: "unknown".into(), suggestion: None };
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
        NashellError::CommandNotFound { name, suggestion } => {
            let mut desc = format!("找不到命令 '{}'", name);
            if let Some(ref sug) = suggestion {
                desc.push_str(&format!(
                    "\n{} 你是不是想输入 '{}' ？",
                    green("Hint:"),
                    green(sug)
                ));
            }
            desc.push_str(&format!(
                "\n{} 使用 {} 查询所有已注册命令",
                green("Hint:"),
                green("!@NaCmds:")
            ));
            ("命令未找到", desc)
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
        NashellError::NaLevelError {
            command,
            used_level,
            expected_level,
        } => {
            let hint = format!(
                "\n{} 请使用 {} 前缀调用此命令（正确格式: {}{}:<args>）",
                green("Hint:"),
                green(expected_level),
                expected_level,
                command
            );
            (
                "调用级别错误",
                format!(
                    "命令 '{}' 是 {} 级命令，但使用了 {} 前缀{}",
                    command, expected_level, used_level, hint
                ),
            )
        }
        NashellError::NaFormatError { input: _, detail, hint, .. } => {
            let hint_text = format!("\n{} {}", green("Hint:"), green(hint));
            ("NaCommand 格式错误", format!("{}{}", detail, hint_text))
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
            suggestion: None,
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n命令未找到:"));
        assert!(formatted.contains("unknown_cmd"));
        assert!(formatted.contains("!@NaCmds:"));
    }

    #[test]
    fn test_format_command_not_found_with_suggestion() {
        let err = NashellError::CommandNotFound {
            name: "NaCmd".into(),
            suggestion: Some("nacmds".into()),
        };
        let formatted = format_error(&err);
        assert!(formatted.contains("NaCmd"));
        assert!(formatted.contains("nacmds"));
        assert!(formatted.contains("你是不是想输入"));
        assert!(formatted.contains("!@NaCmds:"));
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

    #[test]
    fn test_format_na_level_error() {
        let err = NashellError::NaLevelError {
            command: "bash".into(),
            used_level: "!@".into(),
            expected_level: "!!@".into(),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\n调用级别错误:"));
        assert!(formatted.contains("bash"));
        assert!(formatted.contains("Hint:"));
        assert!(formatted.contains("!!@"));
    }

    #[test]
    fn test_format_na_format_error() {
        let err = NashellError::NaFormatError {
            input: "!!@Bash ls".into(),
            detail: "缺少冒号 ':' —— NaCommand 格式应为 <prefix>命令名:<参数>".into(),
            hint: "正确格式: !!@Bash:<bash 参数>".into(),
            cmd_name: Some("Bash".into()),
            used_prefix: Some("!!@".into()),
        };
        let formatted = format_error(&err);
        assert!(formatted.starts_with("@Error #>>\nNaCommand 格式错误:"));
        assert!(formatted.contains("缺少冒号"));
        assert!(formatted.contains("Hint:"));
    }
}
