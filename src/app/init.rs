use std::process::Command;

use crate::error::NashellError;

/// 检测系统中可用的 shell 类型。
///
/// 优先检测 `nu`，若不可用则回退到 `bash`。
/// 检测方法：尝试运行 `{shell} --version`。
///
/// # 返回
/// `"nu"` 或 `"bash"` 字符串。
///
/// # 错误
/// - nu 和 bash 都不可用
pub fn detect_shell_type() -> Result<String, NashellError> {
    // 优先检测 nu
    if Command::new("nu")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        log::info!("检测到 nushell 可用，使用 nu 作为主 shell");
        return Ok("nu".to_string());
    }

    // 回退到 bash
    if Command::new("bash")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        log::info!("使用 bash 作为主 shell");
        return Ok("bash".to_string());
    }

    Err(NashellError::Execute {
        command: "shell_detect".to_string(),
        exit_code: None,
        stderr: "未找到可用的 shell（nu 或 bash）".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_type() {
        let result = detect_shell_type();
        assert!(result.is_ok());
        let shell = result.unwrap();
        assert!(
            shell == "nu" || shell == "bash",
            "shell should be nu or bash, got: {}",
            shell
        );
    }
}
