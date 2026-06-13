pub mod shell_exec;

use crate::error::NashellError;
use crate::parser::syntax::{CmdType, RawCmd};

/// 执行上下文，包含执行所需的依赖。
pub struct ExecContext {
    /// 当前使用的 shell 类型（"bash" 或 "nu"）
    pub shell_type: String,
    /// 管道前一级的输出（用于管道传递）
    pub pre_out: Option<String>,
}

/// 分派命令到对应的执行器。
///
/// Shell 命令通过 `-c` 模式执行。`cd` 命令由 Rust 进程直接处理以保持目录状态。
/// NaCommand 分支在 Phase 5 完善。
///
/// # 参数
/// - `cmd`: 解析后的命令
/// - `ctx`: 执行上下文
pub fn dispatch(cmd: &RawCmd, ctx: &mut ExecContext) -> Result<String, NashellError> {
    match &cmd.cmd_type {
        CmdType::Shell => {
            // 构建完整命令字符串（cmd + args）
            let mut full_cmd = cmd.cmd.clone();
            for arg in &cmd.args {
                full_cmd.push(' ');
                full_cmd.push_str(arg);
            }
            if let Some(ref pre) = ctx.pre_out {
                full_cmd.push(' ');
                full_cmd.push_str(pre);
            }

            // 拦截 cd 命令：由 Rust 进程直接切换目录，保持状态
            if cmd.cmd == "cd" && cmd.args.len() <= 1 {
                shell_exec::exec_cd(&cmd.args)?;
                return Ok(String::new());
            }

            // -c 捕获执行
            let result = shell_exec::exec_captured(
                &cmd.cmd,
                &cmd.args,
                &ctx.shell_type,
            )?;
            if result.exit_code == 0 {
                Ok(result.stdout)
            } else {
                let mut msg = result.stdout;
                if !result.stderr.is_empty() {
                    msg.push_str(&result.stderr);
                }
                Ok(msg)
            }
        }
        CmdType::Interactive => Err(NashellError::Execute {
            command: cmd.cmd.clone(),
            exit_code: None,
            stderr: "交互命令执行将在 Phase 7 实现".to_string(),
        }),
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

    #[test]
    fn test_dispatch_shell_basic() {
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "echo".to_string(),
            args: vec!["hello_world".to_string()],
        };
        let mut ctx = ExecContext {
            shell_type: "bash".to_string(),
            pre_out: None,
        };
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("hello_world"));
    }

    #[test]
    fn test_dispatch_cd() {
        let old = std::env::current_dir().unwrap();
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "cd".to_string(),
            args: vec!["/tmp".to_string()],
        };
        let mut ctx = ExecContext {
            shell_type: "bash".to_string(),
            pre_out: None,
        };
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(std::env::current_dir().unwrap(), std::path::PathBuf::from("/tmp"));
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_dispatch_interactive_not_implemented() {
        let cmd = RawCmd {
            cmd_type: CmdType::Interactive,
            cmd: "vim".to_string(),
            args: vec![],
        };
        let mut ctx = ExecContext {
            shell_type: "bash".to_string(),
            pre_out: None,
        };
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
        let mut ctx = ExecContext {
            shell_type: "bash".to_string(),
            pre_out: None,
        };
        let result = dispatch(&cmd, &mut ctx);
        assert!(result.is_err());
    }
}
