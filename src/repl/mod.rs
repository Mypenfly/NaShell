pub mod input;
pub mod prompt;

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::config::alias;
use crate::config::schema::NashellConfig;
use crate::error::display::format_error;
use crate::executor::{self, ExecContext, OutputType};
use crate::nacommand::registry::CommandRegistry;
use crate::parser;
use crate::parser::syntax::CmdType;
use crate::plugin::broadcast::broadcast_event;
use crate::shell::manager::ShellManager;
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

/// 打印带有类型标识的输出（captured 模式用）。
///
/// Shell 命令输出前显示 `@nu #>>` 或 `@bash #>>`，
/// NaCommand 输出前显示 `@System #>>`，
/// Bash 命令使用亮黄色 `Bash:` 标识。
fn print_captured_output(
    output: &str,
    shell_type: &str,
    config: &NashellConfig,
    output_type: OutputType,
) {
    if output.is_empty() {
        return;
    }
    let mut stdout = std::io::stdout();

    match output_type {
        OutputType::Bash => {
            let prefix = prompt::colorize(
                "Bash:",
                &config.prompts.bash_output_prompt_fg,
            );
            let _ = writeln!(stdout, "{}", prefix);
        }
        OutputType::NaCommand => {
            let prefix = prompt::colorize(
                &config.prompts.output_prompt_format,
                &config.prompts.output_prompt_fg,
            );
            let _ = writeln!(stdout, "{}", prefix);
        }
        OutputType::Shell => {
            let prefix = prompt::colorize(
                &format!("@{} #>>", shell_type),
                &config.prompts.shell_type_fg,
            );
            let _ = writeln!(stdout, "{}", prefix);
        }
    }

    let _ = writeln!(stdout, "{}", output.trim_end());
}

/// 判断命令是否应使用直连终端模式（Stdio::inherit）。
///
/// 条件：单一命令、无 @/Async、非 !!@Bash:、Shell 类型。
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
    // Shell 走直连模式
    matches!(cmd.cmd_type, CmdType::Shell)
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

/// 若 cwd 发生变更则广播 cwd_changed 事件。
fn broadcast_cwd_if_changed(
    old_cwd: &std::path::PathBuf,
    plugin_manager: &Option<Arc<Mutex<crate::plugin::manager::PluginManager>>>,
) {
    let new_cwd = input::current_dir();
    if new_cwd == *old_cwd {
        return;
    }
    if let Some(ref pm) = plugin_manager {
        let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
        let mut handles: Vec<&mut crate::plugin::manager::PluginHandle> =
            mgr.handles_mut().collect();
        let payload = serde_json::json!({"path": new_cwd.to_string_lossy()});
        let _ = broadcast_event("cwd_changed", &payload, &mut handles);
    }
}

/// 广播 shell_state_changed 事件，包含当前所有 shell 的状态快照。
fn broadcast_shell_state(
    plugin_manager: &Option<Arc<Mutex<crate::plugin::manager::PluginManager>>>,
    shell_manager: &Arc<Mutex<ShellManager>>,
) {
    if let Some(ref pm) = plugin_manager {
        let payload = {
            let mgr = shell_manager.lock().unwrap_or_else(|e| e.into_inner());
            let shells: Vec<serde_json::Value> = mgr
                .list_shells()
                .iter()
                .map(|(name, id, path, pools_count)| {
                    serde_json::json!({
                        "name": name,
                        "id": id,
                        "path": path,
                        "pools_count": pools_count,
                    })
                })
                .collect();
            serde_json::json!({ "shells": shells })
        };
        let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
        let mut handles: Vec<&mut crate::plugin::manager::PluginHandle> =
            mgr.handles_mut().collect();
        let _ = broadcast_event("shell_state_changed", &payload, &mut handles);
    }
}

/// 运行 REPL（Read-Eval-Print Loop）循环。
///
/// 显示提示符，读取用户输入，解析并执行，循环直到用户输入 `exit` 或 EOF。
///
/// # 参数
/// - `home_dir`: 用户 home 目录路径
/// - `config`: 完整配置
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
/// - `registry`: 命令注册表
/// - `shell_manager`: Shell 管理器（Arc<Mutex> 共享引用）
pub fn run(
    home_dir: Option<std::path::PathBuf>,
    config: &NashellConfig,
    shell_type: &str,
    registry: CommandRegistry,
    shell_manager: Arc<Mutex<ShellManager>>,
    plugin_manager: Option<Arc<Mutex<crate::plugin::manager::PluginManager>>>,
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

        let old_cwd = input::current_dir();

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

                // === 异步执行 ===
                // 若 @/Async(name) 存在，跳过同步执行，
                // 在后台线程中走完整解析→分派流程异步运行命令，立即返回确认信息。
                if let Some(ref async_name) = raw_commands.async_name {
                    let cwd = input::current_dir();

                    let result = executor::async_exec::spawn_async_shell_exec(
                        async_name,
                        &raw_commands,
                        shell_type,
                        config.shell.timeout_secs,
                        &config.safety.deny_patterns,
                        &shell_manager,
                        registry.clone(),
                        plugin_manager.clone(),
                        &cwd,
                    );

                    match result {
                        Ok(info) => {
                            let cmd_desc = raw_commands
                                .commands
                                .iter()
                                .map(|c| {
                                    let mut s = c.cmd.clone();
                                    for arg in &c.args {
                                        s.push(' ');
                                        s.push_str(arg);
                                    }
                                    s
                                })
                                .collect::<Vec<_>>()
                                .join(" | ");
                            let msg = format!(
                                "shell created and exec: {}\n  name: {}    id: {}",
                                cmd_desc, info.name, info.id
                            );
                            writeln!(stdout, "{}", msg).ok();
                        }
                        Err(e) => {
                            let formatted = format_error(&e);
                            writeln!(stdout, "{}", formatted).ok();
                        }
                    }
                    // 同步 main shell cwd
                    let cwd = input::current_dir();
                    shell_manager.lock().ok().map(|mut m| m.sync_main_cwd(&cwd));
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
                    {
                        let cwd = input::current_dir();
                        shell_manager.lock().ok().map(|mut m| m.sync_main_cwd(&cwd));
                    }
                    continue;
                }

                // === Captured 模式：有管道 / 异步 / Bash ===
                let mut pre_out: Option<String> = None;
                let cmd_count = raw_commands.commands.len();
                let mut last_output_type = OutputType::Shell;

                // 纯 shell 管道（无 NaCommand）：合并为单条 shell -c 执行，
                // 让 shell 原生管道机制处理数据传递，避免逐段拆分后 stdin 断开。
                let all_shell = raw_commands.commands.iter().all(|c| matches!(c.cmd_type, CmdType::Shell));
                if all_shell && cmd_count > 1 {
                    let combined = raw_commands.commands.iter()
                        .map(|c| {
                            let mut s = c.cmd.clone();
                            for arg in &c.args {
                                s.push(' ');
                                s.push_str(arg);
                            }
                            s
                        })
                        .collect::<Vec<_>>()
                        .join(" | ");

                    if let Err(e) = executor::check_safety(&combined, &config.safety.deny_patterns) {
                        let formatted = format_error(&e);
                        writeln!(stdout, "{}", formatted).ok();
                        continue;
                    }

                    match executor::shell_exec::exec_captured(
                        &combined, &[], shell_type, config.shell.timeout_secs, None,
                    ) {
                        Ok(result) => {
                            let mut output = result.stdout;
                            if !result.stderr.is_empty() {
                                if !output.is_empty() {
                                    output.push('\n');
                                }
                                output.push_str(&result.stderr);
                            }
                            if !output.is_empty() {
                                print_captured_output(&output, shell_type, config, OutputType::Shell);
                            }
                        }
                        Err(e) => {
                            let formatted = format_error(&e);
                            writeln!(stdout, "{}", formatted).ok();
                        }
                    }
                    continue;
                }

                for (i, cmd) in raw_commands.commands.iter().enumerate() {
                    let is_last = i == cmd_count - 1;
                    let is_nacmd = matches!(cmd.cmd_type, CmdType::NaCommandNormal | CmdType::NaCommandSystem);
                    let mut ctx = ExecContext {
                        shell_type: shell_type.to_string(),
                        pre_out: pre_out.clone(),
                        timeout_secs: config.shell.timeout_secs,
                        deny_patterns: config.safety.deny_patterns.clone(),
                        long_argument: if i == 0 {
                            raw_commands.long_argument.clone()
                        } else if is_nacmd {
                            pre_out.clone()
                        } else {
                            None
                        },
                        registry: Some(registry.clone()),
                        shell_manager: Some(shell_manager.clone()),
                        plugin_manager: plugin_manager.clone(),
                        config_dir: config.config_dir.clone(),
                    };

                    match executor::dispatch(cmd, &mut ctx, &mut stdout) {
                        Ok((output, output_type)) => {
                            pre_out = Some(output);
                            if is_last {
                                last_output_type = output_type;
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
                    print_captured_output(output, shell_type, config, last_output_type);
                }

                // 若管道中包含 shell 管理命令，广播 shell 状态变更
                let has_shell_cmd = raw_commands.commands.iter().any(|c| {
                    matches!(c.cmd_type, CmdType::NaCommandSystem) && c.cmd == "shell"
                });
                if has_shell_cmd {
                    broadcast_shell_state(&plugin_manager, &shell_manager);
                }
            }
            Err(e) => {
                let formatted = format_error(&e);
                writeln!(stdout, "{}", formatted).ok();
            }
        }

        // 同步 main shell cwd（处理 Switch 等命令导致的 cwd 变更）
        {
            let cwd = input::current_dir();
            shell_manager.lock().ok().map(|mut m| m.sync_main_cwd(&cwd));
        }

        // 广播 cwd 变更事件
        broadcast_cwd_if_changed(&old_cwd, &plugin_manager);

        let _ = stdout.flush();
    }
}
