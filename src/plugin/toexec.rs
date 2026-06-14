use crate::constants::TOEXEC_MAX_DEPTH;
use crate::error::NashellError;
use crate::parser;
use crate::nacommand::registry::CommandRegistry;
use std::sync::{Arc, Mutex};
use crate::shell::manager::ShellManager;
use crate::executor::ExecContext;

/// 执行插件 toExec 中的命令列表。
///
/// 按顺序逐条执行命令，每条命令走完整的用户级解析+分派流程（与用户在 REPL 中输入命令完全一致）。
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

        for raw_cmd in &parsed.commands {
            use crate::parser::syntax::CmdType;

            // 深度检查：NaCommand 在超过深度时拒绝
            if depth_exceeded {
                match &raw_cmd.cmd_type {
                    CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
                        results.push(format!(
                            "@Error #>>\ntoExec 递归深度超过限制 ({}), NaCommand '{}' 被拒绝",
                            TOEXEC_MAX_DEPTH, raw_cmd.cmd
                        ));
                        continue;
                    }
                    CmdType::Shell => {}
                }
            }

            // 使用 dispatch() — 与用户输入完全相同的执行流程
            let mut ctx = ExecContext {
                shell_type: shell_type.to_string(),
                pre_out: parsed.pre_out.clone(),
                timeout_secs,
                deny_patterns: deny_patterns.to_vec(),
                long_argument: parsed.long_argument.clone(),
                registry: Some(registry.clone()),
                shell_manager: shell_manager.clone(),
                plugin_manager: None,
            };

            let mut out_buf = Vec::new();
            match crate::executor::dispatch(raw_cmd, &mut ctx, &mut out_buf) {
                Ok((output, _output_type)) => {
                    let mut combined = String::from_utf8_lossy(&out_buf).into_owned();
                    if !output.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str(&output);
                    }
                    results.push(combined);
                }
                Err(e) => {
                    let err_msg = crate::error::display::format_error(&e);
                    results.push(err_msg);
                }
            }
        }
    }

    Ok(results)
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
        ).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("hello_world"));
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
        ).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_execute_toplevel_parse_error_in_result() {
        let registry = test_registry();
        let to_exec = vec!["'unclosed quote".to_string()];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
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
            "!@Write: @/\nno_path".to_string(),  // Missing path - should fail but not crash
        ];
        let results = execute_toplevel(
            &to_exec, 0, "bash", 120, &[], &registry, None,
        ).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("write to"));
        assert!(results[1].contains("@Error"));
        assert_eq!(std::fs::read_to_string(&file1).unwrap(), "content");
    }
}
