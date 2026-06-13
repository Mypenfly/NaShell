use std::process::{Command, Stdio};

use crate::error::NashellError;

/// 捕获命令执行的结果。
#[derive(Debug, Clone)]
pub struct CapturedOutput {
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 退出码
    pub exit_code: i32,
}

/// 对命令文本做单引号转义，用于嵌入 `shell -c '...'`。
///
/// 规则：将内部 `'` 替换为 `'\''`（结束引用 → 转义单引号 → 恢复引用）。
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 通过 `script -q -c` 在 PTY 中执行 shell 命令并捕获输出。
///
/// `script` 分配伪终端。传入真实 stdin 使 `script` 能查询终端尺寸
///（解决默认 0×0 导致 nushell 表格渲染失败的问题）。
/// 命令结束后 `script` 自动退出，无持久会话、无哨兵、无状态机。
///
/// 执行链路：
///   Rust Command → script -e -q -c "{shell} -c '{command}'" /dev/null
///
/// # 参数
/// - `cmd`: 命令名
/// - `args`: 命令参数
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
pub fn exec_captured(
    cmd: &str,
    args: &[String],
    shell_type: &str,
) -> Result<CapturedOutput, NashellError> {
    let mut full_cmd = cmd.to_string();
    for arg in args {
        full_cmd.push(' ');
        full_cmd.push_str(arg);
    }

    let inner_cmd = format!("{} -c {}", shell_type, shell_quote(&full_cmd));

    let output = Command::new("script")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-e")
        .arg("-q")
        .arg("-c")
        .arg(&inner_cmd)
        .arg("/dev/null")
        .env("TERM", "xterm-256color")
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?
        .wait_with_output()
        .map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(CapturedOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// 执行 `cd` 目录切换。
///
/// 由 Rust 进程直接调用 `std::env::set_current_dir()`，
/// 避免 `bash -c cd` 在不同进程中执行导致状态无法持久的问题。
///
/// # 参数
/// - `args`: cd 的参数，`args[0]` 为目标路径。空参数时切换到 home 目录。
pub fn exec_cd(args: &[String]) -> Result<(), NashellError> {
    let target = if args.is_empty() {
        dirs::home_dir().unwrap_or_else(|| "/".into())
    } else {
        let path = &args[0];
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path == "~" {
                    home
                } else if path.starts_with("~/") {
                    home.join(&path[2..])
                } else {
                    std::path::PathBuf::from(path)
                }
            } else {
                std::path::PathBuf::from(path)
            }
        } else {
            std::path::PathBuf::from(path)
        }
    };

    std::env::set_current_dir(&target).map_err(|e| NashellError::Io {
        path: Some(target.display().to_string()),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_quote_simple() {
        assert_eq!(shell_quote("hello"), "'hello'");
    }

    #[test]
    fn test_shell_quote_with_single_quote() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_exec_captured_basic() {
        let result = exec_captured("echo", &["hello".to_string()], "bash");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("hello"));
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn test_exec_captured_error() {
        let result =
            exec_captured("ls", &["/nonexistent_path_xyz".to_string()], "bash");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_ne!(output.exit_code, 0);
    }

    #[test]
    fn test_exec_captured_nonexistent_command() {
        let result = exec_captured("nonexistent_command_xyz", &[], "bash");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_ne!(output.exit_code, 0);
        assert!(!output.stderr.is_empty() || output.exit_code == 127);
    }

    #[test]
    fn test_exec_cd_tmp() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&["/tmp".to_string()]);
        assert!(result.is_ok());
        assert_eq!(std::env::current_dir().unwrap(), std::path::PathBuf::from("/tmp"));
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_exec_cd_home() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&["~".to_string()]);
        assert!(result.is_ok());
        let home = dirs::home_dir().unwrap();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_exec_cd_empty() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&[]);
        assert!(result.is_ok());
        let home = dirs::home_dir().unwrap();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&old).ok();
    }
}
