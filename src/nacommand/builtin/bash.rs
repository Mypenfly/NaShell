use crate::error::NashellError;
use crate::executor::shell_exec;
use crate::nacommand::cmd::NaCommand;

/// 执行 Bash 命令。
///
/// 将 NaCommand 的 args 拼接为 `bash -c` 的参数并执行。
/// 当 mode 为 "help" 时返回帮助信息。
/// 若 `pre_out` 有值（管道前段输出），通过 `echo | bash -c` 传入。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构（cmd="bash", 可能含 mode="help"）
/// - `pre_out`: 管道前一级的输出（可选）
/// - `timeout_secs`: 超时秒数
///
/// # 返回
/// - `Ok(String)`: 命令执行结果
/// - `Err(NashellError)`: 执行错误
pub fn execute_bash(
    cmd: &NaCommand,
    pre_out: Option<String>,
    timeout_secs: u64,
) -> Result<String, NashellError> {
    // Help 模式
    if cmd.mode.as_deref().map_or(false, |m| m == "help") {
        return Ok(build_help_text());
    }

    let mut bash_args = cmd.args.join(" ");

    // 若有管道前段输出，通过 echo 管道传入
    if let Some(ref pre) = pre_out {
        if !pre.is_empty() {
            let escaped = pre.replace('\\', "\\\\").replace('\'', "'\\''");
            bash_args = format!("echo '{}' | {}", escaped, bash_args);
        }
    }

    let result = shell_exec::exec_bash(&bash_args, timeout_secs)?;

    let mut msg = result.stdout;
    if !result.stderr.is_empty() {
        if !msg.is_empty() {
            msg.push('\n');
        }
        msg.push_str(&result.stderr);
    }

    Ok(msg)
}

/// 构建 Bash 命令的帮助文本（带 ANSI 颜色）。
fn build_help_text() -> String {
    let c = |s: &str| format!("\x1b[96m\x1b[1m{}\x1b[0m", s);
    let h = |s: &str| format!("\x1b[94m{}\x1b[0m", s);
    let g = |s: &str| format!("\x1b[32m{}\x1b[0m", s);
    let y = |s: &str| format!("\x1b[93m{}\x1b[0m", s);

    format!(
        "{}\n  \
         使用 bash 执行命令。适用于 nushell 无法处理的场景，\n  \
         或需要临时回退到 bash 语法的场景。\n\n  {}  \
         \n    args    传递给 bash -c 的参数字符串\n\n  {}  \
         \n    {}\n    {}\n\n  {}  \
         \n  !!@Bash: 的优先级高于所有其他解析规则（管道、@/ 等）。",
        c("Bash"),
        h("参数:"),
        h("使用示例:"),
        g("!!@Bash: ls -la"),
        g("!!@Bash: find . -name '*.rs' | wc -l"),
        y("注意:"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nacommand::cmd::{NaCommand, NaLevel};

    #[test]
    fn test_execute_bash_echo() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "bash".to_string(),
            mode: None,
            args: vec!["echo hello_bash_test".to_string()],
            long_argument: None,
        };
        let result = execute_bash(&cmd, None, 120).unwrap();
        assert!(result.contains("hello_bash_test"));
    }

    #[test]
    fn test_execute_bash_help() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "bash".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };
        let result = execute_bash(&cmd, None, 120).unwrap();
        assert!(result.contains("Bash"));
        assert!(result.contains("bash"));
        assert!(result.contains("nushell"));
    }

    #[test]
    fn test_execute_bash_with_pre_out() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "bash".to_string(),
            mode: None,
            args: vec!["wc -w".to_string()],
            long_argument: None,
        };
        let pre_out = Some("hello world".to_string());
        let result = execute_bash(&cmd, pre_out, 120).unwrap();
        // wc -w 应该输出 "2" (两个单词)
        assert!(result.contains('2'));
    }

    #[test]
    fn test_execute_bash_multiple_args() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "bash".to_string(),
            mode: None,
            args: vec![
                "echo".to_string(),
                "hello".to_string(),
                "world".to_string(),
            ],
            long_argument: None,
        };
        let result = execute_bash(&cmd, None, 120).unwrap();
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn test_execute_bash_nonexistent() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "bash".to_string(),
            mode: None,
            args: vec!["nonexistent_cmd_xyz".to_string()],
            long_argument: None,
        };
        let result = execute_bash(&cmd, None, 120).unwrap();
        assert!(!result.is_empty());
    }
}
