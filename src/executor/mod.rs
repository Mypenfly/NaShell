pub mod async_exec;
pub mod shell_exec;

use std::sync::{Arc, Mutex};

use crate::error::NashellError;
use crate::parser::syntax::{CmdType, RawCmd};
use crate::nacommand::cmd::{NaCommand, NaLevel};
use crate::nacommand::registry::CommandRegistry;
use crate::shell::manager::ShellManager;

/// 命令输出的类型标识。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputType {
    /// 普通 shell 命令
    Shell,
    /// Bash 命令 (!!@Bash:)
    Bash,
    /// NaCommand (内置/外部/插件)
    NaCommand,
}

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
    /// @/ 之后的长参数内容
    pub long_argument: Option<String>,
    /// 命令注册表
    pub registry: Option<CommandRegistry>,
    /// Shell 管理器（用于 Shell 管理命令和异步执行）
    pub shell_manager: Option<Arc<Mutex<ShellManager>>>,
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

/// 从 RawCmd 构建 NaCommand 数据结构。
///
/// 根据命令类型设置 NaLevel。命令名统一转小写。
///
/// **Mode 提取规则**（查表法）：
/// - 从 registry 中查找该命令的 `known_modes`（小写）。
/// - 若 args[0] 匹配已知 mode（大小写不敏感），提取为 `NaCommand.mode` 并从 args 移除。
/// - 若不匹配，args[0] 保持为普通参数。
/// - 外部/插件命令的 `known_modes` 为空，不做 mode 提取，args 原样透传。
///
/// # 参数
/// - `cmd`: 解析后的命令
/// - `long_argument`: 长参数内容
/// - `registry`: 命令注册表（用于查已知 mode 列表）
fn build_nacommand(
    cmd: &RawCmd,
    long_argument: Option<String>,
    registry: &CommandRegistry,
) -> NaCommand {
    let level = match cmd.cmd_type {
        CmdType::NaCommandNormal => NaLevel::Normal,
        CmdType::NaCommandSystem => NaLevel::System,
        _ => NaLevel::Normal,
    };

    let lower_cmd = cmd.cmd.to_lowercase();

    // 查表：该命令的已知 mode 列表
    let known_modes = registry
        .lookup(&lower_cmd)
        .map(|meta| meta.known_modes.clone())
        .unwrap_or_default();

    // 查表法提取 mode：args[0] 匹配已知 mode → mode，否则 → arg
    let (mode, args) = if !known_modes.is_empty() && !cmd.args.is_empty() {
        let first_lower = cmd.args[0].to_lowercase();
        if known_modes.iter().any(|m| m.to_lowercase() == first_lower) {
            (Some(first_lower), cmd.args[1..].to_vec())
        } else {
            (None, cmd.args.clone())
        }
    } else {
        (None, cmd.args.clone())
    };

    NaCommand {
        level,
        cmd: lower_cmd,
        mode,
        args,
        long_argument,
    }
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
pub fn dispatch(cmd: &RawCmd, ctx: &mut ExecContext) -> Result<(String, OutputType), NashellError> {
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
                return Ok((String::new(), OutputType::Shell));
            }

            // -c 捕获执行
            let result = shell_exec::exec_captured(
                &cmd.cmd,
                &cmd.args,
                &ctx.shell_type,
                ctx.timeout_secs,
            )?;
            if result.exit_code == 0 {
                Ok((result.stdout, OutputType::Shell))
            } else {
                let mut msg = result.stdout;
                if !result.stderr.is_empty() {
                    if !msg.is_empty() {
                        msg.push('\n');
                    }
                    msg.push_str(&result.stderr);
                }
                Ok((msg, OutputType::Shell))
            }
        }
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
            Ok((msg, OutputType::Bash))
        }
        CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
            let registry = ctx.registry.as_ref().ok_or_else(|| {
                NashellError::CommandNotFound {
                    name: cmd.cmd.clone(),
                }
            })?;
            let nacmd = build_nacommand(cmd, ctx.long_argument.clone(), registry);
            let output = crate::nacommand::execute_nacommand(
                &nacmd,
                ctx.pre_out.clone(),
                registry,
                ctx.shell_manager.clone(),
                ctx.timeout_secs,
            )?;
            Ok((output, OutputType::NaCommand))
        }
    }
}

/// 直连终端模式执行（不捕获输出）。
///
/// 适用于：单一命令、无管道、无 @/Async、非 !!@Bash:。
/// Shell 命令通过 `exec_shell_direct` 直连终端执行，
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
        // NaCommand 不支持直连模式：内置命令通过 captured 路径执行（有格式化输出前缀），
        // 外部/插件命令同理。若执行到此分支，说明 should_use_direct 判定逻辑有误。
        CmdType::NaCommandNormal | CmdType::NaCommandSystem => {
            Err(NashellError::Execute {
                command: cmd.cmd.clone(),
                exit_code: None,
                stderr: format!(
                    "NaCommand '{}' 不支持直连终端模式，请通过 captured 路径执行",
                    cmd.cmd
                ),
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
            long_argument: None,
            registry: None,
            shell_manager: None,
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
        let (output, output_type) = dispatch(&cmd, &mut ctx).unwrap();
        assert!(output.contains("hello_world"));
        assert!(matches!(output_type, OutputType::Shell));
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
        let (output, output_type) = dispatch(&cmd, &mut ctx).unwrap();
        assert!(output.contains("hello_from_bash"));
        assert!(matches!(output_type, OutputType::Bash));
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

    #[test]
    fn test_build_nacommand_normal_with_help() {
        // args[0]="Help" 匹配 known_modes=["help"] → 提取为 mode
        let mut registry = CommandRegistry::new();
        registry.register_builtin(crate::app::CmdMeta {
            level: crate::app::Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandNormal,
            cmd: "Write".to_string(),
            args: vec!["Help".to_string()],
        };
        let nacmd = build_nacommand(&cmd, None, &registry);
        assert_eq!(nacmd.cmd, "write");
        assert_eq!(nacmd.mode.as_deref(), Some("help"));
        assert!(nacmd.args.is_empty());
    }

    #[test]
    fn test_build_nacommand_normal_with_path_arg() {
        // args[0]="./test.txt" 不匹配 known_modes=["help"] → 保持为 arg
        let mut registry = CommandRegistry::new();
        registry.register_builtin(crate::app::CmdMeta {
            level: crate::app::Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandNormal,
            cmd: "Write".to_string(),
            args: vec!["./test.txt".to_string()],
        };
        let nacmd = build_nacommand(&cmd, None, &registry);
        assert_eq!(nacmd.cmd, "write");
        assert_eq!(nacmd.mode, None);
        assert_eq!(nacmd.args, vec!["./test.txt"]);
    }

    #[test]
    fn test_build_nacommand_normal_with_plain_filename() {
        // flake.nix 不在 known_modes 中 → 不提取为 mode
        let mut registry = CommandRegistry::new();
        registry.register_builtin(crate::app::CmdMeta {
            level: crate::app::Level::Normal,
            name: "open".to_string(),
            exec: "n_open".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandNormal,
            cmd: "Open".to_string(),
            args: vec!["flake.nix".to_string()],
        };
        let nacmd = build_nacommand(&cmd, None, &registry);
        assert_eq!(nacmd.cmd, "open");
        assert_eq!(nacmd.mode, None);
        assert_eq!(nacmd.args, vec!["flake.nix"]);
    }

    #[test]
    fn test_build_nacommand_system_with_mode() {
        // NaCommandSystem 同样查表：args[0]="Watch" 匹配 known_modes → mode
        let mut registry = CommandRegistry::new();
        registry.register_builtin(crate::app::CmdMeta {
            level: crate::app::Level::System,
            name: "shell".to_string(),
            exec: "n_shell".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![
                "watch".to_string(),
                "destroy".to_string(),
                "switch".to_string(),
            ],
        });
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandSystem,
            cmd: "Shell".to_string(),
            args: vec!["Watch".to_string(), "-i".to_string(), "abc".to_string()],
        };
        let nacmd = build_nacommand(&cmd, None, &registry);
        assert_eq!(nacmd.cmd, "shell");
        assert_eq!(nacmd.mode.as_deref(), Some("watch"));
        assert_eq!(nacmd.args, vec!["-i", "abc"]);
        assert!(matches!(nacmd.level, NaLevel::System));
    }
}
