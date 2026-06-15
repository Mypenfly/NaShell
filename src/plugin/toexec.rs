use crate::constants::TOEXEC_MAX_DEPTH;
use crate::error::NashellError;
use crate::executor::shell_exec;
use crate::executor::ExecContext;
use crate::nacommand::registry::CommandRegistry;
use crate::parser;
use crate::shell::manager::ShellManager;
use std::sync::{Arc, Mutex};

/// 执行插件 toExec 中的命令列表。
///
/// 每条命令按顺序执行。执行模式按是否有管道区分：
/// - 无管道：Shell 命令走直连模式（实时输出），NaCommand 走 captured dispatch
/// - 有管道：全管道走 captured dispatch 逐段传递
///
/// 深度计数防止无限递归：超过 TOEXEC_MAX_DEPTH 后，NaCommand 报错拒绝，仅允许纯 shell 命令。
///
/// # 参数
/// - `to_exec`: 要执行的命令字符串列表
/// - `depth`: 当前递归深度（首次调用传 0 或 1）
/// - `shell_type`: 当前 shell 类型（"bash" 或 "nu"）
/// - `timeout_secs`: 命令超时秒数
/// - `deny_patterns`: 安全拦截模式列表
/// - `registry`: 命令注册表
/// - `shell_manager`: Shell 管理器
///
/// # 返回
/// 每条命令的执行结果（顺序与 to_exec 对应）
pub fn execute_toplevel(
    to_exec: &[String],
    depth: u32,
    shell_type: &str,
    timeout_secs: u64,
    deny_patterns: &[String],
    registry: &CommandRegistry,
    shell_manager: Option<Arc<Mutex<ShellManager>>>,
    is_print: bool,
) -> Result<Vec<String>, NashellError> {
    let mut results = Vec::with_capacity(to_exec.len());
    let depth_exceeded = depth >= TOEXEC_MAX_DEPTH;

    for cmd_line in to_exec {
        // 安全拦截检查
        for pattern in deny_patterns {
            if cmd_line.contains(pattern.as_str()) {
                return Err(NashellError::SafetyBlocked {
                    command: cmd_line.clone(),
                    reason: format!("匹配禁止模式: '{}'", pattern),
                });
            }
        }

        let parsed = match parser::parse(cmd_line) {
            Ok(p) => p,
            Err(e) => {
                results.push(format!("@Error #>>\n解析错误: {}", e));
                continue;
            }
        };

        if parsed.commands.is_empty() {
            continue;
        }

        let has_pipes = parsed.commands.len() > 1;

        if !has_pipes {
            // 无管道：单命令执行
            let raw_cmd = &parsed.commands[0];
            results.push(execute_single_direct(
                raw_cmd,
                cmd_line,
                &parsed,
                shell_type,
                timeout_secs,
                deny_patterns,
                registry,
                shell_manager.as_ref(),
                depth_exceeded,
                is_print,
            ));
        } else {
            // 有管道：captured 逐段 dispatch
            results.push(execute_pipeline_captured(
                &parsed,
                cmd_line,
                shell_type,
                timeout_secs,
                deny_patterns,
                registry,
                shell_manager.as_ref(),
                depth_exceeded,
            ));
        }
    }

    Ok(results)
}

/// 执行单个无管道命令（根据 is_print 选择流式或纯捕获模式）。
fn execute_single_direct(
    raw_cmd: &crate::parser::syntax::RawCmd,
    cmd_line: &str,
    parsed: &crate::parser::syntax::RawCommands,
    shell_type: &str,
    timeout_secs: u64,
    deny_patterns: &[String],
    registry: &CommandRegistry,
    shell_manager: Option<&Arc<Mutex<ShellManager>>>,
    depth_exceeded: bool,
    is_print: bool,
) -> String {
    use crate::parser::syntax::CmdType;

    // 深度检查
    if depth_exceeded {
        match &raw_cmd.cmd_type {
            CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
                return format!(
                    "@Error #>>\ntoExec 递归深度超过限制 ({}), NaCommand '{}' 被拒绝",
                    TOEXEC_MAX_DEPTH, raw_cmd.cmd
                );
            }
            CmdType::Shell => {}
        }
    }

    match &raw_cmd.cmd_type {
        CmdType::Shell => {
            let annotation = annotate_cmd(cmd_line);
            if raw_cmd.cmd == "cd" && raw_cmd.args.len() <= 1 {
                match shell_exec::exec_cd(&raw_cmd.args) {
                    Ok(()) => format!("{annotation}\n(direct: cd)"),
                    Err(e) => crate::error::display::format_error(&e),
                }
            } else if is_print {
                // is_print=true: 流式捕获——实时输出到终端同时收集结果
                match shell_exec::exec_captured_streaming(
                    &raw_cmd.cmd, &raw_cmd.args, shell_type, timeout_secs,
                ) {
                    Ok(output) => {
                        let mut text = output.stdout;
                        if !output.stderr.is_empty() {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&output.stderr);
                        }
                        format!("{annotation}\n{}", text.trim())
                    }
                    Err(e) => crate::error::display::format_error(&e),
                }
            } else {
                // is_print=false: 纯捕获——不输出到终端，只收集结果
                match shell_exec::exec_captured(
                    &raw_cmd.cmd, &raw_cmd.args, shell_type, timeout_secs, None,
                ) {
                    Ok(output) => {
                        let mut text = output.stdout;
                        if !output.stderr.is_empty() {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&output.stderr);
                        }
                        format!("{annotation}\n{}", text.trim())
                    }
                    Err(e) => crate::error::display::format_error(&e),
                }
            }
        }
        _ => {
            // NaCommand：走 captured dispatch
            let mut ctx = ExecContext {
                shell_type: shell_type.to_string(),
                pre_out: parsed.pre_out.clone(),
                timeout_secs,
                deny_patterns: deny_patterns.to_vec(),
                long_argument: parsed.long_argument.clone(),
                registry: Some(registry.clone()),
                shell_manager: shell_manager.cloned(),
                plugin_manager: None,
                config_dir: None,
            };
            let mut out_buf = Vec::new();
            match crate::executor::dispatch(raw_cmd, &mut ctx, &mut out_buf) {
                Ok((output, _)) => {
                    let mut combined = String::from_utf8_lossy(&out_buf).into_owned();
                    if !output.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str(&output);
                    }
                    let annotation = annotate_cmd(cmd_line);
                    format!("{annotation}\n{}", combined.trim())
                }
                Err(e) => crate::error::display::format_error(&e),
            }
        }
    }
}

/// 执行管道命令（captured 逐段 dispatch）。
fn execute_pipeline_captured(
    parsed: &crate::parser::syntax::RawCommands,
    cmd_line: &str,
    shell_type: &str,
    timeout_secs: u64,
    deny_patterns: &[String],
    registry: &CommandRegistry,
    shell_manager: Option<&Arc<Mutex<ShellManager>>>,
    depth_exceeded: bool,
) -> String {
    use crate::parser::syntax::CmdType;

    let _cmd_count = parsed.commands.len();
    let mut pre_out: Option<String> = None;

    for (i, raw_cmd) in parsed.commands.iter().enumerate() {
        // 深度检查
        if depth_exceeded {
            match &raw_cmd.cmd_type {
                CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
                    return format!(
                        "@Error #>>\ntoExec 递归深度超过限制 ({}), NaCommand '{}' 被拒绝",
                        TOEXEC_MAX_DEPTH, raw_cmd.cmd
                    );
                }
                CmdType::Shell => {}
            }
        }

        let mut ctx = ExecContext {
            shell_type: shell_type.to_string(),
            pre_out: pre_out.clone(),
            timeout_secs,
            deny_patterns: deny_patterns.to_vec(),
            long_argument: if i == 0 {
                parsed.long_argument.clone()
            } else {
                None
            },
            registry: Some(registry.clone()),
            shell_manager: shell_manager.cloned(),
            plugin_manager: None,
            config_dir: None,
        };

        let mut out_buf = Vec::new();
        match crate::executor::dispatch(raw_cmd, &mut ctx, &mut out_buf) {
            Ok((output, _)) => {
                pre_out = Some(output);
            }
            Err(e) => {
                return crate::error::display::format_error(&e);
            }
        }
    }

    let annotation = annotate_cmd(cmd_line);
    if let Some(output) = pre_out {
        format!("{annotation}\n{}", output.trim())
    } else {
        annotation
    }
}

/// 生成嵌套提示符标注命令来源。
fn annotate_cmd(cmd_line: &str) -> String {
    crate::repl::prompt::colorize(&format!("  @[{}] #>", cmd_line), "dark_gray")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CmdMeta, Level};
    use crate::nacommand::registry::CommandRegistry;
    use std::io::Write;

    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    fn setup_temp_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nashell_toexec_{}_{}", std::process::id(), id));
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
        registry
    }

    #[test]
    fn test_execute_toplevel_shell_command() {
        let registry = test_registry();
        let to_exec = vec!["echo hello_world".to_string()];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("hello_world"));
    }

    #[test]
    fn test_execute_toplevel_pipeline_uses_captured() {
        let registry = test_registry();
        let to_exec = vec!["echo hello | grep hello".to_string()];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        // Captured mode: shows actual output
        assert!(results[0].contains("hello"));
    }

    #[test]
    fn test_execute_toplevel_na_command() {
        let dir = setup_temp_dir();
        let file_path = dir.join("toexec_test.txt");
        let registry = test_registry();

        let to_exec = vec![format!(
            "!@Write:{} @/\nhello content",
            file_path.to_string_lossy()
        )];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("write to"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello content");
    }

    #[test]
    fn test_execute_toplevel_depth_limit() {
        let registry = test_registry();
        let to_exec = vec!["!@Write:./test.txt @/\ntest".to_string()];
        let results = execute_toplevel(
            &to_exec, TOEXEC_MAX_DEPTH, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("递归深度超过限制"));
    }

    #[test]
    fn test_execute_toplevel_shell_allowed_at_max_depth() {
        let registry = test_registry();
        let to_exec = vec!["echo still_works".to_string()];
        let results = execute_toplevel(
            &to_exec, TOEXEC_MAX_DEPTH, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("still_works"));
    }

    #[test]
    fn test_execute_toplevel_multiple_commands() {
        let dir = setup_temp_dir();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        let registry = test_registry();

        let to_exec = vec![
            format!("!@Write:{} @/\ncontent1", file1.to_string_lossy()),
            format!("!@Write:{} @/\ncontent2", file2.to_string_lossy()),
        ];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("write to"));
        assert!(results[1].contains("write to"));
        assert_eq!(std::fs::read_to_string(&file1).unwrap(), "content1");
        assert_eq!(std::fs::read_to_string(&file2).unwrap(), "content2");
    }

    #[test]
    fn test_execute_toplevel_empty_list() {
        let registry = test_registry();
        let results = execute_toplevel(
            &[], 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_execute_toplevel_parse_error_in_result() {
        let registry = test_registry();
        let to_exec = vec!["'unclosed quote".to_string()];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("@Error"));
        assert!(results[0].contains("解析错误"));
    }

    #[test]
    fn test_execute_toplevel_safety_blocked() {
        let registry = test_registry();
        let to_exec = vec!["rm -rf /".to_string()];
        let deny: Vec<String> = vec!["rm -rf /".to_string()];
        let result = execute_toplevel(
            &to_exec, 0, "bash", 120, &deny, &registry, None,
            true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_toplevel_error_in_result_not_fatal() {
        let dir = setup_temp_dir();
        let file1 = dir.join("file1.txt");
        let registry = test_registry();

        let to_exec = vec![
            format!("!@Write:{} @/\ncontent", file1.to_string_lossy()),
            "!@Write: @/\nno_path".to_string(),
        ];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
            true,
        ).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("write to"));
        assert!(results[1].contains("@Error"));
        assert_eq!(std::fs::read_to_string(&file1).unwrap(), "content");
    }
}
