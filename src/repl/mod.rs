pub mod input;
pub mod prompt;

use std::io::Write;
use std::process::{Command, Stdio};

use crate::config::alias;
use crate::config::schema::NashellConfig;
use crate::error::display::format_error;
use crate::executor::{self, ExecContext};
use crate::parser;
use crate::parser::syntax::CmdType;
use rustyline::DefaultEditor;

/// 显示启动时的 opening 内容。
///
/// 按优先级：执行 `opening.exec` 命令 → 显示 `opening.file` 文件内容。
/// 若两者都未配置则无输出。
fn show_opening(config: &NashellConfig) {
    let mut stdout = std::io::stdout();

    if let Some(ref exec_cmd) = config.opening.exec {
        log::info!("Executing opening command: {}", exec_cmd);
        // 使用 Stdio::inherit 使子进程直连终端，保留 ANSI 颜色输出
        match Command::new("sh")
            .arg("-c")
            .arg(exec_cmd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(mut child) => {
                let _ = child.wait();
            }
            Err(e) => {
                log::warn!("Failed to execute opening command '{}': {}", exec_cmd, e);
            }
        }
        return;
    }

    if let Some(ref file_path) = config.opening.file {
        match std::fs::read_to_string(file_path) {
            Ok(content) => {
                let _ = writeln!(stdout, "{}", content.trim_end());
            }
            Err(e) => {
                log::warn!(
                    "Failed to read opening file '{}': {}",
                    file_path,
                    e
                );
            }
        }
    }
}

/// 在命令输出前打印 shell 类型前缀（直连模式用）。
///
/// 颜色来自配置的 `shell_type_fg`。
fn print_shell_prefix(shell_type: &str, config: &NashellConfig) {
    let mut stdout = std::io::stdout();
    let prefix = prompt::colorize(
        &format!("@{} #>>", shell_type),
        &config.prompts.shell_type_fg,
    );
    let _ = writeln!(stdout, "{}", prefix);
}

/// 打印带有 shell 类型标识的输出（captured 模式用）。
///
/// Shell 命令输出前显示 `@nu #>>` 或 `@bash #>>` 标识，
/// 颜色来自配置的 `shell_type_fg`。Bash 命令使用亮黄色 `Bash:` 标识。
fn print_captured_output(
    output: &str,
    shell_type: &str,
    config: &NashellConfig,
    is_bash: bool,
) {
    if output.is_empty() {
        return;
    }
    let mut stdout = std::io::stdout();

    if is_bash {
        let prefix = prompt::colorize(
            "Bash:",
            &config.prompts.bash_output_prompt_fg,
        );
        let _ = writeln!(stdout, "{}", prefix);
    } else {
        let prefix = prompt::colorize(
            &format!("@{} #>>", shell_type),
            &config.prompts.shell_type_fg,
        );
        let _ = writeln!(stdout, "{}", prefix);
    }

    let _ = writeln!(stdout, "{}", output.trim_end());
}

/// 判断命令是否应使用直连终端模式（Stdio::inherit）。
///
/// 条件：单一命令、无 @/Async、非 !!@Bash:、Shell 或 Interactive 类型。
fn should_use_direct(raw_commands: &parser::syntax::RawCommands) -> bool {
    if raw_commands.commands.len() != 1 {
        return false;
    }
    if raw_commands.async_name.is_some() {
        return false;
    }
    let cmd = &raw_commands.commands[0];
    // !!@Bash: → captured 模式（需要亮黄色标识）
    if matches!(cmd.cmd_type, CmdType::NaCommandSystem) && cmd.cmd == "bash" {
        return false;
    }
    // Shell 和 Interactive 走直连模式
    matches!(cmd.cmd_type, CmdType::Shell | CmdType::Interactive)
}

/// 构建命令字符串（用于安全检查）。
fn build_cmd_string(cmd: &parser::syntax::RawCmd) -> String {
    let mut s = cmd.cmd.clone();
    for arg in &cmd.args {
        s.push(' ');
        s.push_str(arg);
    }
    s
}

/// 运行 REPL（Read-Eval-Print Loop）循环。
///
/// 显示提示符，读取用户输入，解析并执行，循环直到用户输入 `exit` 或 EOF。
///
/// # 参数
/// - `home_dir`: 用户 home 目录路径
/// - `config`: 完整配置
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
pub fn run(
    home_dir: Option<std::path::PathBuf>,
    config: &NashellConfig,
    shell_type: &str,
) {
    let mut stdout = std::io::stdout();
    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            log::error!("Failed to initialize line editor: {}", e);
            return;
        }
    };

    log::info!("REPL started with shell type: {}", shell_type);

    show_opening(config);

    loop {
        let cwd = input::current_dir();
        let home = home_dir.as_deref();

        let prompts = &config.prompts;
        let prompt_str = prompt::colorize(
            &prompt::generate_prompt(&cwd, home, &prompts.input_prompt_format),
            &prompts.input_prompt_fg,
        );

        let input = match input::read_multiline(
            &mut rl,
            &prompt_str,
            &prompts.input_continue_format,
        ) {
            Ok(Some(line)) => line,
            Ok(None) => {
                writeln!(stdout).ok();
                break;
            }
            Err(e) => {
                log::error!("Failed to read input: {}", e);
                continue;
            }
        };

        if input == "exit" {
            log::info!("User requested exit");
            break;
        }

        if input.is_empty() {
            continue;
        }

        // 展开别名（在解析之前）
        let expanded = if config.aliases.is_empty() {
            input
        } else {
            alias::expand_alias(&input, &config.aliases)
        };

        match parser::parse(&expanded) {
            Ok(raw_commands) => {
                log::debug!(
                    "Parsed: {} command(s), long_arg={}, async={:?}",
                    raw_commands.commands.len(),
                    raw_commands.long_argument.is_some(),
                    raw_commands.async_name,
                );

                if raw_commands.commands.is_empty() {
                    continue;
                }

                // === 直连模式：无管道、无异步、非 Bash ===
                if should_use_direct(&raw_commands) {
                    let cmd = &raw_commands.commands[0];
                    let cmd_str = build_cmd_string(cmd);

                    if let Err(e) =
                        executor::check_safety(&cmd_str, &config.safety.deny_patterns)
                    {
                        let formatted = format_error(&e);
                        writeln!(stdout, "{}", formatted).ok();
                        continue;
                    }

                    print_shell_prefix(shell_type, config);

                    match executor::dispatch_direct(cmd, shell_type) {
                        Ok(()) => {}
                        Err(e) => {
                            let formatted = format_error(&e);
                            writeln!(stdout, "{}", formatted).ok();
                        }
                    }
                    continue;
                }

                // === Captured 模式：有管道 / 异步 / Bash ===
                let mut pre_out: Option<String> = None;
                let cmd_count = raw_commands.commands.len();
                let mut last_is_bash = false;

                for (i, cmd) in raw_commands.commands.iter().enumerate() {
                    let is_last = i == cmd_count - 1;
                    let mut ctx = ExecContext {
                        shell_type: shell_type.to_string(),
                        pre_out: pre_out.clone(),
                        timeout_secs: config.shell.timeout_secs,
                        deny_patterns: config.safety.deny_patterns.clone(),
                    };

                    match executor::dispatch(cmd, &mut ctx) {
                        Ok((output, is_shell)) => {
                            pre_out = Some(output);
                            if is_last {
                                last_is_bash = !is_shell
                                    && cmd.cmd == "bash"
                                    && matches!(
                                        cmd.cmd_type,
                                        CmdType::NaCommandSystem
                                    );
                            }
                        }
                        Err(e) => {
                            let formatted = format_error(&e);
                            writeln!(stdout, "{}", formatted).ok();
                            pre_out = None;
                            break;
                        }
                    }
                }

                if let Some(ref output) = pre_out {
                    print_captured_output(output, shell_type, config, last_is_bash);
                }

                if raw_commands.async_name.is_some() {
                    log::debug!(
                        "Async execution requested: {:?}",
                        raw_commands.async_name
                    );
                }
            }
            Err(e) => {
                let formatted = format_error(&e);
                writeln!(stdout, "{}", formatted).ok();
            }
        }

        let _ = stdout.flush();
    }
}
