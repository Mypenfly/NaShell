use std::path::Path;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

/// 使用 rustyline 从终端读取单行输入。
///
/// 显示给定的提示符，等待用户输入一行文本。
///
/// # 参数
/// - `prompt`: 提示符字符串
///
/// # 返回
/// - `Ok(Some(String))`: 成功读取到输入
/// - `Ok(None)`: 用户输入 EOF（Ctrl+D）或中断（Ctrl+C）
/// - `Err`: 读取错误
pub fn read_line(prompt: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    match rl.readline(prompt) {
        Ok(line) => {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                let _ = rl.add_history_entry(line);
            }
            Ok(Some(trimmed))
        }
        Err(ReadlineError::Interrupted) => {
            // Ctrl+C → treat as EOF
            Ok(None)
        }
        Err(ReadlineError::Eof) => {
            // Ctrl+D → exit
            Ok(None)
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// 获取当前工作目录路径。
///
/// # 返回
/// 当前工作目录的 PathBuf
pub fn current_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| Path::new("/").to_path_buf())
}
