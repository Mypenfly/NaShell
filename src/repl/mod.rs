pub mod input;
pub mod prompt;

use std::io::Write;

use crate::config::schema::PromptsConfig;
use crate::error::display::format_error;
use crate::executor::{self, ExecContext};
use crate::parser;
use rustyline::DefaultEditor;

/// 运行 REPL（Read-Eval-Print Loop）循环。
///
/// 显示提示符，读取用户输入，解析并执行，循环直到用户输入 `exit` 或 EOF。
///
/// # 参数
/// - `home_dir`: 用户 home 目录路径
/// - `prompts`: 提示符配置
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
pub fn run(
    home_dir: Option<std::path::PathBuf>,
    prompts: &PromptsConfig,
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

    loop {
        let cwd = input::current_dir();
        let home = home_dir.as_deref();

        let prompt_str =
            prompt::colorize(
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

        match parser::parse(&input) {
            Ok(raw_commands) => {
                log::debug!(
                    "Parsed: {} command(s), long_arg={}, async={:?}",
                    raw_commands.commands.len(),
                    raw_commands.long_argument.is_some(),
                    raw_commands.async_name,
                );

                let mut pre_out: Option<String> = None;
                let cmd_count = raw_commands.commands.len();

                for (i, cmd) in raw_commands.commands.iter().enumerate() {
                    let _is_last = i == cmd_count - 1;
                    let mut ctx = ExecContext {
                        shell_type: shell_type.to_string(),
                        pre_out: pre_out.clone(),
                    };

                    match executor::dispatch(cmd, &mut ctx) {
                        Ok(output) => {
                            pre_out = Some(output);
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
                    writeln!(stdout, "{}", output.trim_end()).ok();
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
