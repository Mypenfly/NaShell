pub mod cmd;
pub mod registry;
pub mod builtin;

use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use crate::nacommand::registry::{CommandRegistry, LookupSource};
use crate::plugin::manager::PluginManager;
use crate::plugin::protocol::PluginCall;
use crate::shell::manager::ShellManager;

/// 执行 NaCommand。
///
/// 查表找到对应的命令处理器并执行。
/// 当 mode 为 "help" 时返回帮助信息。
/// 当前支持的内置命令：write, open, bash, shell。
/// 插件命令通过 PluginManager 的 call/response/off 协议执行。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
/// - `pre_out`: 管道前一级的输出（用于管道传递）
/// - `registry`: 命令注册表
/// - `shell_manager`: Shell 管理器（用于 Shell 管理命令，可选）
/// - `timeout_secs`: 命令超时秒数（用于 Bash 命令）
/// - `plugin_manager`: 插件管理器（用于插件命令，可选）
/// - `shell_type`: 当前 shell 类型
/// - `deny_patterns`: 安全拦截模式列表
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
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
    shell_type: &str,
    deny_patterns: &[String],
    out_writer: &mut dyn Write,
) -> Result<String, NashellError> {
    let lower_cmd = cmd.cmd.to_lowercase();

    // 查表确认命令已注册，并获取来源
    let (_meta, source) = registry.lookup_with_source(&lower_cmd)?;

    // 对内置命令，走原有的快速匹配路径
    if source == LookupSource::Builtin {
        return execute_builtin(cmd, pre_out, registry, shell_manager, timeout_secs);
    }

    // 插件命令：通过 PluginManager 的 call/response/off 协议执行
    if source == LookupSource::Plugin {
        if let Some(ref pm) = plugin_manager {
            let mode = cmd.mode.clone().unwrap_or_else(|| "normal".to_string());
            let level = match cmd.level {
                crate::nacommand::cmd::NaLevel::Normal => "normal",
                crate::nacommand::cmd::NaLevel::System => "system",
            };

            let call = PluginCall {
                command: lower_cmd.clone(),
                mode,
                level: level.to_string(),
                params: cmd.args.clone(),
                long_argument: cmd.long_argument.clone(),
            };

            let plugin_name = registry.command_owner(&lower_cmd)
                .unwrap_or(&"unknown".to_string())
                .clone();

            let mut mgr = pm.lock().map_err(|e| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: format!("无法获取 PluginManager 锁: {}", e),
            })?;

            let handle = mgr.get_handle(&plugin_name)
                .ok_or_else(|| NashellError::Plugin {
                    plugin_name: plugin_name.clone(),
                    detail: "插件句柄未找到，插件可能未启动".to_string(),
                })?;

            PluginManager::send_call(handle, &call)?;

            let (responses, off) = PluginManager::recv_responses(
                handle,
                out_writer,
                shell_type,
                timeout_secs,
                deny_patterns,
                registry,
                shell_manager.clone(),
            )?;

            // Build final output: only non-streaming (off message) content
            // Streaming responses were already written to out_writer by recv_responses.
            let mut output = String::new();
            // Handle off message output
            if off.is_print && !off.out_content.is_empty() {
                if let Some(ref prompt) = off.out_prompt {
                    let _ = writeln!(out_writer, "{}", prompt);
                }
                let _ = writeln!(out_writer, "{}", off.out_content);
            }
            // Execute off to_exec commands
            if !off.to_exec.is_empty() {
                let exec_results = crate::plugin::toexec::execute_toplevel(
                    &off.to_exec,
                    1,
                    shell_type,
                    timeout_secs,
                    deny_patterns,
                    registry,
                    shell_manager,
                )?;
                for result in exec_results {
                    if !result.is_empty() {
                        let _ = writeln!(out_writer, "{}", result);
                    }
                }
            }

            return Ok(output);
        } else {
            return Err(NashellError::Plugin {
                plugin_name: lower_cmd,
                detail: "PluginManager 未初始化，无法执行插件命令".to_string(),
            });
        }
    }

    // Config commands: not yet implemented (Phase 8)
    if source == LookupSource::Config {
        return Err(NashellError::Execute {
            command: lower_cmd,
            exit_code: None,
            stderr: "外部配置命令暂未实现 (Phase 8)".to_string(),
        });
    }

    Err(NashellError::CommandNotFound {
        name: cmd.cmd.clone(),
    })
}

/// 执行内置 NaCommand。
///
/// 快速匹配内置命令名，分发到对应的 handler。
fn execute_builtin(
    cmd: &NaCommand,
    pre_out: Option<String>,
    registry: &CommandRegistry,
    shell_manager: Option<Arc<Mutex<ShellManager>>>,
    timeout_secs: u64,
) -> Result<String, NashellError> {
    let lower_cmd = cmd.cmd.to_lowercase();

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

        let mut out_buf = Vec::new();
        let result = execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf).unwrap();
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

        let mut out_buf = Vec::new();
        let result = strip_ansi(&execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf).unwrap());
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

        let mut out_buf = Vec::new();
        let result = execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf).unwrap();
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

        let mut out_buf = Vec::new();
        let result = execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf);
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

        let mut out_buf = Vec::new();
        let result = execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf).unwrap();
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

        let mut out_buf = Vec::new();
        let result = execute_nacommand(&cmd, None, &registry, None, 120, None, "bash", &[], &mut out_buf).unwrap();
        assert!(result.contains("Open"));
        assert!(result.contains("打开文件"));
    }
}
