use std::path::Path;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

/// 使用复用的 rustyline Editor 读取单行输入。
///
/// 与 `read_line` 不同，此函数接受 `&mut DefaultEditor` 参数，
/// 允许调用方在 REPL 循环中复用同一个 Editor 实例，从而保留
/// 输入历史并避免每次读取时重新初始化终端。
///
/// # 参数
/// - `rl`: 可复用的 rustyline Editor 实例
/// - `prompt`: 提示符字符串
///
/// # 返回
/// - `Ok(Some(String))`: 成功读取到输入
/// - `Ok(None)`: 用户输入 EOF（Ctrl+D）或中断（Ctrl+C）
/// - `Err`: 读取错误
///
/// # 错误
/// 底层 rustyline 的 I/O 错误
pub fn read_line_with_editor(
    rl: &mut DefaultEditor,
    prompt: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match rl.readline(prompt) {
        Ok(line) => {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                let _ = rl.add_history_entry(line);
            }
            Ok(Some(trimmed))
        }
        Err(ReadlineError::Interrupted) => {
            Ok(None)
        }
        Err(ReadlineError::Eof) => {
            Ok(None)
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// 检查输入行是否触发多行输入模式。
///
/// 当首行末尾有 `@/` 截止符（包括 `@/Async(name)`）时返回 true。
///
/// # 参数
/// - `line`: 首行输入
///
/// # 返回
/// 是否触发多行输入模式
pub fn is_multiline_trigger(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.ends_with("@/") {
        return true;
    }
    // 检查末尾是否为 @/Async(name) 模式
    if let Some(pos) = trimmed.rfind("@/Async(") {
        let after_marker = &trimmed[pos..];
        if after_marker.ends_with(')') {
            return true;
        }
    }
    false
}

/// 读取多行输入（当首行以 `@/` 结尾时自动进入续行模式）。
///
/// 首行以 `first_prompt` 提示，续行以 `continue_prompt` 提示。
/// 续行时输入空行（仅回车）或 EOF 结束输入。
///
/// # 参数
/// - `rl`: 复用的 rustyline Editor 实例
/// - `first_prompt`: 首行提示符
/// - `continue_prompt`: 续行提示符
///
/// # 返回
/// - `Ok(Some(String))`: 完整输入（多行以 `\n` 连接）
/// - `Ok(None)`: 用户 EOF 或中断
/// - `Err`: 读取错误
///
/// # 错误
/// 底层 rustyline 的 I/O 错误
pub fn read_multiline(
    rl: &mut DefaultEditor,
    first_prompt: &str,
    continue_prompt: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let first_line = match read_line_with_editor(rl, first_prompt)? {
        Some(line) => line,
        None => return Ok(None),
    };

    if first_line.is_empty() {
        return Ok(Some(String::new()));
    }

    // 检查是否需要进入多行模式
    if !is_multiline_trigger(&first_line) {
        return Ok(Some(first_line));
    }

    let mut lines = vec![first_line];
    loop {
        match read_line_with_editor(rl, continue_prompt)? {
            None => break,
            Some(s) if s.is_empty() => break,
            Some(s) => lines.push(s),
        }
    }

    Ok(Some(lines.join("\n")))
}

/// 获取当前工作目录路径。
///
/// # 返回
/// 当前工作目录的 PathBuf
pub fn current_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| Path::new("/").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_multiline_trigger_at_slash() {
        assert!(is_multiline_trigger("!@Write:./test.py @/"));
    }

    #[test]
    fn test_is_multiline_trigger_async() {
        assert!(is_multiline_trigger("ls -la @/Async(test)"));
    }

    #[test]
    fn test_is_multiline_trigger_no_at_slash() {
        assert!(!is_multiline_trigger("ls -la"));
    }

    #[test]
    fn test_is_multiline_trigger_at_slash_not_at_end() {
        // @/ not at end of line
        assert!(!is_multiline_trigger("ls -la @/ grep"));
    }

    #[test]
    fn test_is_multiline_trigger_async_not_at_end() {
        // @/Async should be at end
        assert!(is_multiline_trigger("ls -la @/Async(test) "));
    }

    #[test]
    fn test_is_multiline_trigger_empty() {
        assert!(!is_multiline_trigger(""));
    }
}
