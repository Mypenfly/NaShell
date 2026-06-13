pub mod input;
pub mod prompt;

use std::io::Write;

/// 运行 REPL（Read-Eval-Print Loop）循环。
///
/// 显示提示符，读取用户输入，处理并循环，直到用户输入 `exit` 或 EOF。
///
/// # 参数
/// - `home_dir`: 用户 home 目录路径（用于 `~` 提示符缩写）
pub fn run(home_dir: Option<std::path::PathBuf>) {
    let mut stdout = std::io::stdout();

    loop {
        // 获取当前工作目录
        let cwd = input::current_dir();
        let home = home_dir.as_deref();

        // 生成提示符
        let prompt = prompt::generate_prompt(&cwd, home);

        // 读取输入
        let input = match input::read_line(&prompt) {
            Ok(Some(line)) => line,
            Ok(None) => {
                // Ctrl+C or Ctrl+D
                writeln!(stdout, "").ok();
                break;
            }
            Err(e) => {
                log::error!("Failed to read input: {}", e);
                continue;
            }
        };

        // 检查退出命令
        if input == "exit" {
            log::info!("User requested exit");
            break;
        }

        // 跳过多余的空行
        if input.is_empty() {
            continue;
        }

        // 占位：回显输入
        writeln!(stdout, "echo: {}", input).ok();
        let _ = stdout.flush();
    }
}
