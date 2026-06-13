pub mod shell_exec;

use crate::error::NashellError;
use crate::parser::syntax::{CmdType, RawCmd};

/// 执行上下文，包含执行所需的依赖。
pub struct ExecContext {
    /// 当前使用的 shell 类型（"bash" 或 "nu"）
    pub shell_type: String,
    /// 管道前一级的输出（用于管道传递）
    pub pre_out: Option<String>,
    /// Shell 命令超时秒数
    pub timeout_secs: u64,
    /// 安全拦截模式列表
    pub deny_patterns: Vec<String>,
}

/// 检查命令是否匹配安全拦截模式。
///
/// 遍历 `deny_patterns`，若命令字符串包含任一模式则返回拦截错误。
///
/// # 参数
/// - `command`: 完整的命令字符串
/// - `patterns`: 禁止的模式列表
///
/// # 返回
/// - `Ok(())`: 命令通过安全检查
/// - `Err(NashellError::SafetyBlocked)`: 命令被拦截
pub(crate) fn check_safety(command: &str, patterns: &[String]) -> Result<(), NashellError> {
    for pattern in patterns {
        if command.contains(pattern.as_str()) {
            return Err(NashellError::SafetyBlocked {
                command: command.to_string(),
                reason: format!("匹配禁止模式: '{}'", pattern),
            });
        }
    }
    Ok(())
}

/// 分派命令到对应的执行器。
///
/// Shell 命令通过 `-c` 模式执行。`cd` 命令由 Rust 进程直接处理以保持目录状态。
/// Bash 命令（`!!@Bash:`）通过 `bash -c` 执行。
/// NaCommand 分支在 Phase 5 完善。
///
/// # 参数
/// - `cmd`: 解析后的命令
/// - `ctx`: 执行上下文
pub fn dispatch(cmd: &RawCmd, ctx: &mut ExecContext) -> Result<(String, bool), NashellError> {
    // 安全拦截检查（在正式执行前）
    let full_cmd_str = {
        let mut s = cmd.cmd.clone();
        for arg in &cmd.args {
            s.push(' ');
            s.push_str(arg);
        }
        if let Some(ref pre) = ctx.pre_out {
            s.push(' ');
            s.push_str(pre);
        }
        s
    };
    check_safety(&full_cmd_str, &ctx.deny_patterns)?;

    match &cmd.cmd_type {
        CmdType::Shell => {
            // 拦截 cd 命令：由 Rust 进程直接切换目录，保持状态
            if cmd.cmd == "cd" && cmd.args.len() <= 1 {
                shell_exec::exec_cd(&cmd.args)?;
                return Ok((String::new(), false));
            }

            // -c 捕获执行
            let result = shell_exec::exec_captured(
                &cmd.cmd,
                &cmd.args,
                &ctx.shell_type,
                ctx.timeout_secs,
            )?;
            if result.exit_code == 0 {
                Ok((result.stdout, true))
            } else {
                let mut msg = result.stdout;
                if !result.stderr.is_empty() {
                    if !msg.is_empty() {
                        msg.push('\n');
                    }
                    msg.push_str(&result.stderr);
                }
                Ok((msg, true))
            }
        }
        CmdType::Interactive => Err(NashellError::Execute {
            command: cmd.cmd.clone(),
            exit_code: None,
            stderr: "交互命令执行将在 Phase 7 实现".to_string(),
        }),
        CmdType::NaCommandSystem if cmd.cmd == "bash" => {
            // !!@Bash: 命令 — 通过 bash -c 执行
            let bash_args = cmd.args.first().cloned().unwrap_or_default();
            let result = shell_exec::exec_bash(&bash_args, ctx.timeout_secs)?;
            let mut msg = result.stdout;
            if !result.stderr.is_empty() {
                if !msg.is_empty() {
                    msg.push('\n');
                }
                msg.push_str(&result.stderr);
            }
            // 标记为 Bash 输出，以便 REPL 使用亮黄色标识
            Ok((msg, false))
        }
        CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
            Err(NashellError::CommandNotFound {
                name: cmd.cmd.clone(),
            })
        }
    }
}

/// 直连终端模式执行（不捕获输出）。
///
/// 适用于：单一命令、无管道、无 @/Async、非 !!@Bash:。
/// Shell/Interactive 命令通过 `exec_shell_direct` 直连终端执行，
/// 子进程 stdin/stdout/stderr 全部继承，支持实时输出和交互输入。
/// `cd` 仍由 Rust 进程拦截处理以保持目录状态。
/// NaCommand 命令回退到 captured 模式。
///
/// # 参数
/// - `cmd`: 解析后的命令
/// - `shell_type`: shell 类型
pub fn dispatch_direct(cmd: &RawCmd, shell_type: &str) -> Result<(), NashellError> {
    match &cmd.cmd_type {
        CmdType::Shell => {
            if cmd.cmd == "cd" && cmd.args.len() <= 1 {
                return shell_exec::exec_cd(&cmd.args);
            }
            shell_exec::exec_shell_direct(&cmd.cmd, &cmd.args, shell_type)?;
            Ok(())
        }
        CmdType::Interactive => {
            shell_exec::exec_shell_direct(&cmd.cmd, &cmd.args, shell_type)?;
            Ok(())
        }
        // NaCommand 在直连模式下回退——内置命令是 Rust 代码、外部命令待 Phase 9 支持
        CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
            Err(NashellError::CommandNotFound {
                name: cmd.cmd.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::syntax::RawCmd;

    fn test_ctx() -> ExecContext {
        ExecContext {
            shell_type: "bash".to_string(),
            pre_out: None,
            timeout_secs: 120,
            deny_patterns: Vec::new(),
        }
    }

    #[test]
    fn test_dispatch_shell_basic() {
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "echo".to_string(),
            args: vec!["hello_world".to_string()],
        };
        let mut ctx = test_ctx();
        let (output, is_shell) = dispatch(&cmd, &mut ctx).unwrap();
        assert!(output.contains("hello_world"));
        assert!(is_shell);
    }

    #[test]
    fn test_dispatch_cd() {
        let old = std::env::current_dir().unwrap();
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "cd".to_string(),
            args: vec!["/tmp".to_string()],
        };
        let mut ctx = test_ctx();
        let (output, _) = dispatch(&cmd, &mut ctx).unwrap();
        assert!(output.is_empty());
        assert_eq!(
            std::env::current_dir().unwrap(),
            std::path::PathBuf::from("/tmp")
        );
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_dispatch_interactive_not_implemented() {
        let cmd = RawCmd {
            cmd_type: CmdType::Interactive,
            cmd: "vim".to_string(),
            args: vec![],
        };
        let mut ctx = test_ctx();
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatch_nacommand_not_found() {
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandNormal,
            cmd: "UnknownCmd".to_string(),
            args: vec![],
        };
        let mut ctx = test_ctx();
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatch_bash_command() {
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandSystem,
            cmd: "bash".to_string(),
            args: vec!["echo hello_from_bash".to_string()],
        };
        let mut ctx = test_ctx();
        let (output, is_bash) = dispatch(&cmd, &mut ctx).unwrap();
        assert!(output.contains("hello_from_bash"));
        assert!(!is_bash); // Bash 命令返回 false 表示非普通 shell
    }

    #[test]
    fn test_safety_check_blocked() {
        let mut ctx = test_ctx();
        ctx.deny_patterns = vec!["rm -rf /".to_string()];
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "rm".to_string(),
            args: vec!["-rf".to_string(), "/".to_string()],
        };
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_err());
        match result {
            Err(NashellError::SafetyBlocked { .. }) => {}
            _ => panic!("expected SafetyBlocked error"),
        }
    }

    #[test]
    fn test_safety_check_allowed() {
        let mut ctx = test_ctx();
        ctx.deny_patterns = vec!["rm -rf /".to_string()];
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "echo".to_string(),
            args: vec!["hello".to_string()],
        };
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_ok());
    }
}
