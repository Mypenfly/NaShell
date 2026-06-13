mod app;
mod config;
mod constants;
mod error;
mod executor;
mod nacommand;
mod parser;
mod repl;
mod shell;

use app::AppData;

fn main() {
    env_logger::init();

    log::info!("NaShell starting...");

    let config = match config::loader::load_config(None) {
        Ok(cfg) => {
            log::debug!(
                "Config: prompts.input_prompt_fg={:?}",
                cfg.prompts.input_prompt_fg
            );
            log::debug!(
                "Config: prompts.input_prompt_format={:?}",
                cfg.prompts.input_prompt_format
            );
            log::debug!("Config: shell.timeout_secs={}", cfg.shell.timeout_secs);
            log::debug!(
                "Config: plugins.max_recursion_depth={}",
                cfg.plugins.max_recursion_depth
            );
            log::debug!("Config: na_commands count={}", cfg.na_commands.len());
            log::debug!("Config: aliases count={}", cfg.aliases.len());
            log::debug!(
                "Config: safety.deny_patterns count={}",
                cfg.safety.deny_patterns.len()
            );
            log::info!("Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            log::error!("Fatal: failed to load configuration: {}", e);
            return;
        }
    };

    let _app_data = AppData::default();

    // 检测 shell 类型
    let shell_type = match app::init::detect_shell_type() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Fatal: no shell available: {}", e);
            return;
        }
    };

    let home_dir = dirs::home_dir();

    // 进入 REPL 循环
    repl::run(home_dir, &config, &shell_type);

    log::info!("NaShell exiting.");
}
