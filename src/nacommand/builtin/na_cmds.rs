use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::app::Level;
use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use crate::nacommand::registry::CommandRegistry;
use crate::plugin::manager::PluginManager;

/// 命令条目的来源标签。
#[derive(Debug, Clone, Copy, PartialEq)]
enum CmdSource {
    Builtin,
    Config,
    Plugin,
}

impl CmdSource {
    fn as_str(&self) -> &'static str {
        match self {
            CmdSource::Builtin => "Builtin",
            CmdSource::Config => "Config",
            CmdSource::Plugin => "Plugin",
        }
    }
}

/// 单条命令的展示信息。
struct CmdInfo {
    name: String,
    level: String,
    source: CmdSource,
    plugin_name: String,
}

/// 收集所有已注册命令的信息。
fn collect_cmd_infos(registry: &CommandRegistry) -> Vec<CmdInfo> {
    let mut infos = Vec::new();

    for meta in &registry.builtin_cmds {
        infos.push(CmdInfo {
            name: meta.name.clone(),
            level: level_str(&meta.level),
            source: CmdSource::Builtin,
            plugin_name: String::new(),
        });
    }
    for meta in &registry.config_cmds {
        infos.push(CmdInfo {
            name: meta.name.clone(),
            level: level_str(&meta.level),
            source: CmdSource::Config,
            plugin_name: String::new(),
        });
    }
    for meta in &registry.plugin_cmds {
        let owner = registry
            .command_owner(&meta.name)
            .cloned()
            .unwrap_or_default();
        infos.push(CmdInfo {
            name: meta.name.clone(),
            level: level_str(&meta.level),
            source: CmdSource::Plugin,
            plugin_name: owner,
        });
    }

    infos
}

/// 将 Level 转为字符串。
fn level_str(level: &Level) -> String {
    match level {
        Level::Normal => "Normal".to_string(),
        Level::System => "System".to_string(),
    }
}

/// 解析 json 标志。
fn parse_json_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "-j" || a == "--json")
}

/// 输出表格。
fn format_table(infos: &[CmdInfo]) -> String {
    if infos.is_empty() {
        return "NaCommands registry\n  (暂无命令)".to_string();
    }

    let max_name = infos.iter().map(|c| c.name.len()).max().unwrap_or(4).max(4);
    let max_level = infos.iter().map(|c| c.level.len()).max().unwrap_or(5).max(5);
    let max_source = 7; // "Builtin".len()
    let max_plugin = infos
        .iter()
        .map(|c| c.plugin_name.len())
        .max()
        .unwrap_or(6)
        .max(6);

    let mut out = String::from("NaCommands registry\n");
    out.push_str(&format!(
        "  {:<width_name$}  {:<width_level$}  {:<width_source$}  {:<width_plugin$}\n",
        "name",
        "level",
        "source",
        "plugin",
        width_name = max_name,
        width_level = max_level,
        width_source = max_source,
        width_plugin = max_plugin,
    ));

    for info in infos {
        out.push_str(&format!(
            "  {:<width_name$}  {:<width_level$}  {:<width_source$}  {:<width_plugin$}\n",
            info.name,
            info.level,
            info.source.as_str(),
            info.plugin_name,
            width_name = max_name,
            width_level = max_level,
            width_source = max_source,
            width_plugin = max_plugin,
        ));
    }

    out.pop(); // 移除末尾换行
    out
}

/// 剥离 ANSI 转义序列。
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_digit() || nc == ';' {
                    chars.next();
                } else {
                    break;
                }
            }
            chars.next();
            continue;
        }
        result.push(c);
    }
    result
}

/// 输出 JSON。
fn format_json(infos: &[CmdInfo]) -> Result<String, NashellError> {
    let commands: Vec<serde_json::Value> = infos
        .iter()
        .map(|info| {
            serde_json::json!({
                "name": info.name,
                "level": info.level,
                "source": info.source.as_str(),
                "plugin": info.plugin_name,
            })
        })
        .collect();

    let json = serde_json::json!({
        "version": "1.0",
        "commands": commands,
    });

    serde_json::to_string_pretty(&json).map_err(|e| NashellError::Execute {
        command: "nacmds".to_string(),
        exit_code: None,
        stderr: format!("JSON 序列化失败: {}", e),
    })
}

/// 执行 NaCmds 命令：列出所有已注册命令的信息。
///
/// 支持模式：
/// - 默认（无 mode）：表格形式列出命令名、级别、来源
/// - Detail：详细模式，额外包含帮助信息
/// - Help：返回帮助信息
///
/// 支持选项：
/// - --json / -j：以 JSON 格式输出
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
/// - `registry`: 命令注册表
/// - `plugin_manager`: 插件管理器（detail 模式获取插件帮助）
/// - `config_dir`: 配置文件目录（detail 模式执行外部命令 --help）
///
/// # 返回
/// - `Ok(String)`: 命令注册表信息
/// - `Err(NashellError)`: 执行错误
pub fn execute_na_cmds(
    cmd: &NaCommand,
    registry: &CommandRegistry,
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
    config_dir: Option<&Path>,
) -> Result<String, NashellError> {
    let mode = cmd.mode.as_deref();
    let json_output = parse_json_flag(&cmd.args);

    if mode == Some("help") {
        return Ok(build_help_text());
    }

    // Detail 模式
    if mode == Some("detail") {
        let infos = collect_cmd_infos(registry);

        // 获取每条命令的帮助摘要
        let mut detail_infos: Vec<serde_json::Value> = Vec::new();
        for info in &infos {
            let help_text = match info.source {
                CmdSource::Builtin => {
                    registry.get_help(&info.name, None).unwrap_or_else(|_| String::new())
                }
                CmdSource::Config => {
                    // 通过执行外部程序 --help 获取帮助
                    get_config_help(&info.name, registry, config_dir)
                }
                CmdSource::Plugin => {
                    // 通过插件协议获取帮助
                    get_plugin_help(&info.name, registry, &plugin_manager)
                }
            };

            // 取帮助文本的首行作为摘要（表格模式用）
            let summary = help_text.lines().next().unwrap_or("").to_string();

            let help_value = if json_output {
                strip_ansi(&help_text)
            } else {
                summary
            };

            detail_infos.push(serde_json::json!({
                "name": info.name,
                "level": info.level,
                "source": info.source.as_str(),
                "plugin": info.plugin_name,
                "help": help_value,
            }));
        }

        if json_output {
            let json = serde_json::json!({
                "version": "1.0",
                "commands": detail_infos,
            });
            return serde_json::to_string_pretty(&json).map_err(|e| NashellError::Execute {
                command: "nacmds".to_string(),
                exit_code: None,
                stderr: format!("JSON 序列化失败: {}", e),
            });
        } else {
            return Ok(format_detail_table(&detail_infos));
        }
    }

    let infos = collect_cmd_infos(registry);

    if json_output {
        let raw = format_json(&infos)?;
        Ok(strip_ansi(&raw))
    } else {
        Ok(format_table(&infos))
    }
}

/// 构建 NaCmds 命令的帮助文本。
fn build_help_text() -> String {
    let c = |s: &str| format!("\x1b[96m\x1b[1m{}\x1b[0m", s);
    let h = |s: &str| format!("\x1b[94m{}\x1b[0m", s);
    let g = |s: &str| format!("\x1b[32m{}\x1b[0m", s);

    format!(
        "{}\n  \
         列出所有已注册的 NaCommand（内置 / 用户配置 / 插件）。\n\n  {}  \
         \n    NaCmds:          默认模式，表格输出命令名、级别、来源\n    \
         NaCmds:Detail     详细模式，包含帮助信息\n    \
         NaCmds:Help       显示此帮助\n\n  {}  \
         \n    -j / --json      以 JSON 格式输出\n\n  {}  \
         \n    {}\n    {}",
        c("NaCmds"),
        h("模式:"),
        h("选项:"),
        h("使用示例:"),
        g("!@NaCmds:"),
        g("!@NaCmds:Detail -j"),
    )
}

/// 获取外部配置命令的帮助（通过执行 --help）。
fn get_config_help(
    cmd_name: &str,
    registry: &CommandRegistry,
    config_dir: Option<&Path>,
) -> String {
    let cmd_meta = match registry.lookup(cmd_name) {
        Ok(meta) => meta,
        Err(_) => return String::new(),
    };

    let nacmd = NaCommand {
        level: crate::nacommand::cmd::NaLevel::Normal,
        cmd: cmd_name.to_string(),
        mode: Some("help".to_string()),
        args: vec![],
        long_argument: None,
    };

    match crate::nacommand::external::execute_external(cmd_meta, &nacmd, config_dir) {
        Ok(output) => output,
        Err(e) => format!("(帮助获取失败: {})", e),
    }
}

/// 获取插件命令的帮助（通过 call/response 协议）。
fn get_plugin_help(
    cmd_name: &str,
    registry: &CommandRegistry,
    plugin_manager: &Option<Arc<Mutex<PluginManager>>>,
) -> String {
    let plugin_name = match registry.command_owner(cmd_name) {
        Some(name) => name.clone(),
        None => return format!("(插件 {} 所属插件未找到)", cmd_name),
    };

    if let Some(ref pm) = plugin_manager {
        let mut mgr = match pm.lock() {
            Ok(m) => m,
            Err(e) => return format!("(无法获取 PluginManager 锁: {})", e),
        };

        let handle = match mgr.get_handle(&plugin_name) {
            Some(h) => h,
            None => return format!("(插件 {} 未启动)", plugin_name),
        };

        match PluginManager::get_command_help(handle, cmd_name) {
            Ok(help) => help,
            Err(e) => format!("(帮助获取失败: {})", e),
        }
    } else {
        "(PluginManager 未初始化)".to_string()
    }
}

/// 格式化 detail 模式表格。
fn format_detail_table(detail_infos: &[serde_json::Value]) -> String {
    if detail_infos.is_empty() {
        return "NaCommands detail\n  (暂无命令)".to_string();
    }

    let max_name = detail_infos
        .iter()
        .map(|d| d["name"].as_str().unwrap_or("").len())
        .max()
        .unwrap_or(4)
        .max(4);
    let max_level = 5;
    let max_source = 7;

    let mut out = String::from("NaCommands detail\n");
    out.push_str(&format!(
        "  {:<width_name$}  {:<width_level$}  {:<width_source$}  help_summary\n",
        "name",
        "level",
        "source",
        width_name = max_name,
        width_level = max_level,
        width_source = max_source,
    ));

    for d in detail_infos {
        let name = d["name"].as_str().unwrap_or("");
        let level = d["level"].as_str().unwrap_or("");
        let source = d["source"].as_str().unwrap_or("");
        let help = d["help"].as_str().unwrap_or("");

        // 截断帮助摘要到 60 字符
        let summary: String = if help.len() > 60 {
            format!("{}...", &help[..60])
        } else {
            help.to_string()
        };

        out.push_str(&format!(
            "  {:<width_name$}  {:<width_level$}  {:<width_source$}  {}\n",
            name,
            level,
            source,
            summary,
            width_name = max_name,
            width_level = max_level,
            width_source = max_source,
        ));
    }

    out.pop();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CmdMeta, Level};
    use crate::nacommand::cmd::{NaCommand, NaLevel};

    fn test_registry() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        registry.builtin_cmds.push(CmdMeta {
            level: Level::System,
            name: "shell".to_string(),
            exec: "n_shell".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec!["watch".to_string(), "help".to_string()],
        });
        registry.config_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "websearch".to_string(),
            exec: "python3 ./web_search.py".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });
        registry
    }

    #[test]
    fn test_na_cmds_default_table() {
        let registry = test_registry();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: None,
            args: vec![],
            long_argument: None,
        };

        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        assert!(result.contains("NaCommands registry"));
        assert!(result.contains("write"));
        assert!(result.contains("shell"));
        assert!(result.contains("websearch"));
        assert!(result.contains("Builtin"));
        assert!(result.contains("Config"));
    }

    #[test]
    fn test_na_cmds_json_flag_variants() {
        let registry = test_registry();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: None,
            args: vec!["--json".to_string()],
            long_argument: None,
        };
        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        assert!(result.contains("\"version\""));
    }

    #[test]
    fn test_na_cmds_detail_with_builtin_help() {
        let registry = test_registry();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: Some("detail".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        // detail 模式应包含命令名和帮助摘要
        assert!(result.contains("NaCommands detail"));
        assert!(result.contains("write"));
        assert!(result.contains("shell"));
        assert!(result.contains("websearch"));
        // 内置命令应包含帮助信息的首行摘要
        assert!(result.contains("写入文件") || result.contains("help"));
    }

    #[test]
    fn test_na_cmds_detail_json() {
        let registry = test_registry();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: Some("detail".to_string()),
            args: vec!["-j".to_string()],
            long_argument: None,
        };

        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        assert!(result.contains("\"version\""));
        assert!(result.contains("\"help\""));
    }

    #[test]
    fn test_na_cmds_help() {
        let registry = test_registry();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        assert!(result.contains("NaCmds"));
        assert!(result.contains("Detail"));
    }

    #[test]
    fn test_na_cmds_empty_registry() {
        let registry = CommandRegistry::new();
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "nacmds".to_string(),
            mode: None,
            args: vec![],
            long_argument: None,
        };

        let result = execute_na_cmds(&cmd, &registry, None, None).unwrap();
        assert!(result.contains("暂无命令"));
    }
}
