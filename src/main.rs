mod app;
mod config;
mod constants;
mod error;
mod executor;
mod nacommand;
mod parser;
mod plugin;
mod repl;
mod shell;

use std::sync::{Arc, Mutex};

use app::{CmdMeta, Level};
use nacommand::registry::CommandRegistry;
use plugin::manager::PluginManager;
use shell::manager::ShellManager;

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

    // 构建内置命令注册表
    let mut registry = CommandRegistry::new();
    // Normal 级命令
    registry.register_builtin(CmdMeta {
        level: Level::Normal,
        name: "write".to_string(),
        exec: "n_write".to_string(),
        long_argument: true,
        exec_script: None,
        known_modes: vec!["help".to_string()],
    });
    registry.register_builtin(CmdMeta {
        level: Level::Normal,
        name: "open".to_string(),
        exec: "n_open".to_string(),
        long_argument: false,
        exec_script: None,
        known_modes: vec!["help".to_string()],
    });
    // System 级命令
    registry.register_builtin(CmdMeta {
        level: Level::System,
        name: "bash".to_string(),
        exec: "n_bash".to_string(),
        long_argument: false,
        exec_script: None,
        known_modes: vec!["help".to_string()],
    });
    registry.register_builtin(CmdMeta {
        level: Level::System,
        name: "shell".to_string(),
        exec: "n_shell".to_string(),
        long_argument: false,
        exec_script: None,
        known_modes: vec![
            "watch".to_string(),
            "destroy".to_string(),
            "switch".to_string(),
            "help".to_string(),
        ],
    });

    let _app_data = app::AppData::default();

    // 检测 shell 类型
    let shell_type = match app::init::detect_shell_type() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Fatal: no shell available: {}", e);
            return;
        }
    };

    // 创建 ShellManager
    let mut shell_manager = ShellManager::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
    shell_manager.register_main(&cwd);
    let shell_manager = Arc::new(Mutex::new(shell_manager));

    // 初始化插件系统
    let plugins_dir = plugin::manifest::expand_plugins_dir(&config.plugins.dir);
    let plugin_metas = plugin::manifest::scan_plugins(&plugins_dir);
    log::info!("扫描到 {} 个插件", plugin_metas.len());

    // 加载插件命令到注册表
    registry.load_plugins(&plugin_metas);

    // 创建 PluginManager 并启动 broadcast 插件
    let plugin_manager = if !plugin_metas.is_empty() {
        let mut pm = PluginManager::new();
        let broadcast_plugins: Vec<_> = plugin_metas
            .iter()
            .filter(|p| p.is_broadcast)
            .cloned()
            .collect();

        for meta in &broadcast_plugins {
            match pm.start_plugin(meta.clone()) {
                Ok(name) => log::info!("已启动广播插件: {}", name),
                Err(e) => log::warn!("启动插件 '{}' 失败: {}", meta.name, e),
            }
        }

        // Store non-broadcast plugin metas for lazy start when called
        for meta in &plugin_metas {
            if !meta.is_broadcast {
                if let Err(e) = pm.start_plugin(meta.clone()) {
                    log::warn!("预启动插件 '{}' 失败: {}", meta.name, e);
                }
            }
        }

        Some(Arc::new(Mutex::new(pm)))
    } else {
        None
    };

    let home_dir = dirs::home_dir();

    log::info!("Entering REPL with shell type: {}", shell_type);

    // 进入 REPL 循环
    repl::run(home_dir, &config, &shell_type, registry, shell_manager, plugin_manager);

    log::info!("NaShell exiting.");
}
