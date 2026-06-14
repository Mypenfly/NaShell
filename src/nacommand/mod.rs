pub mod cmd;
pub mod registry;
pub mod builtin;

use std::sync::{Arc, Mutex};

use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use crate::nacommand::registry::CommandRegistry;
use crate::shell::manager::ShellManager;

/// 执行 NaCommand。
///
/// 查表找到对应的命令处理器并执行。
/// 当 mode 为 "help" 时返回帮助信息。
/// 当前支持的内置命令：write, open, bash, shell。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
/// - `pre_out`: 管道前一级的输出（用于管道传递）
/// - `registry`: 命令注册表
/// - `shell_manager`: Shell 管理器（用于 Shell 管理命令，可选）
/// - `timeout_secs`: 命令超时秒数（用于 Bash 命令）
///
/// # 返回
/// - `Ok(String)`: 命令执行结果
/// - `Err(NashellError)`: 命令未找到或执行错误
pub fn execute_nacommand(
    cmd: &NaCommand,
    pre_out: Option<String>,
    registry: &CommandRegistry,
    shell_manager: Option<Arc<Mutex<ShellManager>>>,
    timeout_secs: u64,
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
        "bash" => {
            if cmd.mode.as_deref().map_or(false, |m| m == "help") {
                return registry.get_help("bash", Some("help"));
            }
            builtin::bash::execute_bash(cmd, pre_out, timeout_secs)
        }
        "shell" => {
            if cmd.mode.as_deref().map_or(false, |m| m == "help") {
                return registry.get_help("shell", Some("help"));
            }
            let mgr = shell_manager.ok_or_else(|| NashellError::Execute {
                command: cmd.cmd.clone(),
                exit_code: None,
                stderr: "ShellManager 未初始化".to_string(),
            })?;
            builtin::shell_cmd::execute_shell_cmd(cmd, &mgr)
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

    fn strip_ansi(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() || nc == ';' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                chars.next();
                continue;
            }
            result.push(c);
        }
        result
    }

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

        let result = execute_nacommand(&cmd, None, &registry, None, 120).unwrap();
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

        let result = strip_ansi(&execute_nacommand(&cmd, None, &registry, None, 120).unwrap());
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

        let result = execute_nacommand(&cmd, None, &registry, None, 120).unwrap();
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

        let result = execute_nacommand(&cmd, None, &registry, None, 120);
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

        let result = execute_nacommand(&cmd, None, &registry, None, 120).unwrap();
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

        let result = execute_nacommand(&cmd, None, &registry, None, 120).unwrap();
        assert!(result.contains("Open"));
        assert!(result.contains("打开文件"));
    }
}
