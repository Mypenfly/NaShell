mod config;
mod constants;
mod error;

use config::loader;

fn main() {
    // 初始化日志系统
    env_logger::init();

    log::info!("NaShell starting...");

    // 加载配置
    match loader::load_config(None) {
        Ok(config) => {
            log::debug!("Config: prompts.input_prompt_fg={:?}", config.prompts.input_prompt_fg);
            log::debug!("Config: shell.timeout_secs={}", config.shell.timeout_secs);
            log::debug!("Config: plugins.max_recursion_depth={}", config.plugins.max_recursion_depth);
            log::debug!("Config: na_commands count={}", config.na_commands.len());
            log::debug!("Config: aliases count={}", config.aliases.len());
            log::debug!("Config: safety.deny_patterns count={}", config.safety.deny_patterns.len());
            log::info!("Configuration loaded successfully");
        }
        Err(e) => {
            log::error!("Fatal: failed to load configuration: {}", e);
        }
    }

    log::info!("NaShell exiting.");
}
