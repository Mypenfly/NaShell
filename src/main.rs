mod app;
mod config;
mod constants;
mod error;
mod nacommand;
mod parser;
mod repl;
mod shell;

use app::AppData;

fn main() {
    // 初始化日志系统
    env_logger::init();

    log::info!("NaShell starting...");

    // 加载配置
    let _config = match config::loader::load_config(None) {
        Ok(cfg) => {
            log::debug!("Config: prompts.input_prompt_fg={:?}", cfg.prompts.input_prompt_fg);
            log::debug!("Config: shell.timeout_secs={}", cfg.shell.timeout_secs);
            log::debug!("Config: plugins.max_recursion_depth={}", cfg.plugins.max_recursion_depth);
            log::debug!("Config: na_commands count={}", cfg.na_commands.len());
            log::debug!("Config: aliases count={}", cfg.aliases.len());
            log::debug!("Config: safety.deny_patterns count={}", cfg.safety.deny_patterns.len());
            log::info!("Configuration loaded successfully");
            Some(cfg)
        }
        Err(e) => {
            log::error!("Fatal: failed to load configuration: {}", e);
            None
        }
    };

    // 构造 AppData（目前所有 Vec 为空，Phase 5+ 填充）
    let _app_data = AppData::default();

    // 获取 home 目录用于提示符
    let home_dir = dirs::home_dir();

    // 进入 REPL 循环
    repl::run(home_dir);

    log::info!("NaShell exiting.");
}
