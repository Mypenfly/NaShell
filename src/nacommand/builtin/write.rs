use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use std::fs;
use std::path::PathBuf;

/// 执行 Write 命令：将内容写入文件。
///
/// 提取 path（args[0]）和 content（long_argument）。
/// 检查父目录是否存在，若不存在则报错。
/// long_argument 为 None 时创建空文件或清空既有文件。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
///
/// # 返回
/// - `Ok(String)`: 格式 `write to {abs_path} ({bytes} bytes)`
/// - `Err(NashellError)`: 父目录不存在、路径缺失或 IO 错误
pub fn execute_write(cmd: &NaCommand) -> Result<String, NashellError> {
    let path_str = cmd.args.first().ok_or_else(|| NashellError::Execute {
        command: "write".to_string(),
        exit_code: None,
        stderr: "缺少文件路径参数".to_string(),
    })?;

    let file_path = PathBuf::from(path_str);
    let abs_path = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.clone());

    let parent = file_path.parent();
    match parent {
        Some(p) if !p.as_os_str().is_empty() => {
            if !p.exists() {
                return Err(NashellError::Execute {
                    command: "write".to_string(),
                    exit_code: None,
                    stderr: format!("父目录不存在: {}", p.display()),
                });
            }
        }
        _ => {}
    }

    let content = cmd.long_argument.as_deref().unwrap_or("");
    fs::write(&file_path, content).map_err(|e| NashellError::Io {
        path: Some(path_str.to_string()),
        source: e,
    })?;

    let abs_path_display = if abs_path.exists() {
        abs_path.display().to_string()
    } else {
        // canonicalize failed, use joined path
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&file_path)
            .display()
            .to_string()
    };

    Ok(format!(
        "write to {} ({} bytes)",
        abs_path_display,
        content.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nacommand::cmd::{NaCommand, NaLevel};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// 在临时目录中创建测试环境，返回临时目录路径。
    fn setup_temp_dir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nashell_write_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_write_creates_new_file_with_content() {
        let dir = setup_temp_dir();
        let file_path = dir.join("new_file.txt");

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some("hello world".to_string()),
        };

        let result = execute_write(&cmd).unwrap();
        assert!(result.contains("write to"));
        assert!(result.contains("bytes"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_write_overwrites_existing_file() {
        let dir = setup_temp_dir();
        let file_path = dir.join("existing.txt");
        std::fs::write(&file_path, "old content").unwrap();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some("new content".to_string()),
        };

        let result = execute_write(&cmd).unwrap();
        assert!(result.contains("write to"));
        assert!(result.contains("bytes"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_write_creates_empty_file_when_no_long_argument() {
        let dir = setup_temp_dir();
        let file_path = dir.join("empty.txt");

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: None,
        };

        let result = execute_write(&cmd).unwrap();
        assert!(result.contains("write to"));
        assert!(result.contains("0 bytes"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_write_preserves_multiline_content() {
        let dir = setup_temp_dir();
        let file_path = dir.join("multiline.txt");

        let content = "line1\n  line2 with indent\n    line3 with more indent\n";
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some(content.to_string()),
        };

        execute_write(&cmd).unwrap();
        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn test_write_parent_directory_does_not_exist() {
        let dir = setup_temp_dir();
        let file_path = dir.join("nonexistent_dir").join("file.txt");

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some("content".to_string()),
        };

        let result = execute_write(&cmd);
        assert!(result.is_err());
        match result {
            Err(crate::error::NashellError::Execute { .. }) => {}
            _ => panic!("expected Execute error"),
        }
    }

    #[test]
    fn test_write_no_path_argument() {
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![],
            long_argument: Some("content".to_string()),
        };

        let result = execute_write(&cmd);
        assert!(result.is_err());
    }
}
