pub mod cmd;
pub mod registry;
pub mod builtin;

use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use crate::nacommand::registry::CommandRegistry;

/// 执行 NaCommand。
///
/// 查表找到对应的命令处理器并执行。
/// 当 mode 为 "help" 时返回帮助信息。
/// 当前支持的内置命令：write, open。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
/// - `pre_out`: 管道前一级的输出（用于管道传递，当前未使用）
/// - `registry`: 命令注册表
///
/// # 返回
/// - `Ok(String)`: 命令执行结果
/// - `Err(NashellError)`: 命令未找到或执行错误
pub fn execute_nacommand(
    cmd: &NaCommand,
    _pre_out: Option<String>,
    registry: &CommandRegistry,
) -> Result<String, NashellError> {
    let lower_cmd = cmd.cmd.to_lowercase();

    // 查表确认命令已注册
    let _found = registry.lookup(&lower_cmd)?;

    match lower_cmd.as_str() {
        "write" => {
            if cmd.mode.as_deref().map_or(false, |m| m == "help") {
                return registry.get_help("write", Some("help"));
            }
            builtin::write::execute_write(cmd)
        }
        "open" => {
            if cmd.mode.as_deref().map_or(false, |m| m == "help") {
                return registry.get_help("open", Some("help"));
            }
            builtin::open::execute_open(cmd)
        }
        _ => Err(NashellError::CommandNotFound {
            name: cmd.cmd.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CmdMeta, Level};
    use crate::nacommand::cmd::{NaCommand, NaLevel};
    use crate::nacommand::registry::CommandRegistry;
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn setup_temp_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nashell_exec_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_registry() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "open".to_string(),
            exec: "n_open".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        registry
    }

    #[test]
    fn test_execute_write_command() {
        let dir = setup_temp_dir();
        let file_path = dir.join("test.txt");
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some("hello from execute".to_string()),
        };

        let result = execute_nacommand(&cmd, None, &registry).unwrap();
        assert!(result.contains("write to"));
        assert!(result.contains("bytes"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello from execute");
    }

    #[test]
    fn test_execute_open_command() {
        let dir = setup_temp_dir();
        let file_path = dir.join("test.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        writeln!(f, "line 1").unwrap();
        writeln!(f, "line 2").unwrap();
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "open".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: None,
        };

        let result = execute_nacommand(&cmd, None, &registry).unwrap();
        assert!(result.contains("1  line 1"));
        assert!(result.contains("2  line 2"));
    }

    #[test]
    fn test_execute_help_mode() {
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_nacommand(&cmd, None, &registry).unwrap();
        assert!(result.contains("Write"));
        assert!(result.contains("写入文件"));
    }

    #[test]
    fn test_execute_command_not_found() {
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "unknown".to_string(),
            mode: None,
            args: vec![],
            long_argument: None,
        };

        let result = execute_nacommand(&cmd, None, &registry);
        assert!(result.is_err());
        match result {
            Err(NashellError::CommandNotFound { name }) => {
                assert_eq!(name, "unknown");
            }
            _ => panic!("expected CommandNotFound error"),
        }
    }

    #[test]
    fn test_execute_case_insensitive() {
        let dir = setup_temp_dir();
        let file_path = dir.join("test.txt");
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "Write".to_string(),
            mode: None,
            args: vec![file_path.to_string_lossy().to_string()],
            long_argument: Some("case insensitive".to_string()),
        };

        let result = execute_nacommand(&cmd, None, &registry).unwrap();
        assert!(result.contains("write to"));
    }

    #[test]
    fn test_execute_help_mode_open() {
        let registry = test_registry();

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "open".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_nacommand(&cmd, None, &registry).unwrap();
        assert!(result.contains("Open"));
        assert!(result.contains("打开文件"));
    }
}
