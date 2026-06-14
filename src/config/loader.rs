use crate::config::schema::{ExternalCmdConfig, NashellConfig};
use crate::error::NashellError;

/// 预处理 KDL 文本，将裸 `true`/`false` 转换为 KDL v2 的 `#true`/`#false`。
///
/// KDL v1 允许裸的 `true`/`false` 作为布尔值，而 KDL v2 要求使用 `#true`/`#false`。
/// 此函数确保与 nashell_dev.md 中定义的配置格式兼容。
fn preprocess_bare_bools(content: &str) -> String {
    content
        .replace("=true", "=#true")
        .replace("=false", "=#false")
}

/// 将 KdlValue 转为 String。
fn value_to_string(val: &kdl::KdlValue) -> String {
    match val {
        kdl::KdlValue::String(s) => s.clone(),
        kdl::KdlValue::Integer(i) => i.to_string(),
        kdl::KdlValue::Float(f) => f.to_string(),
        kdl::KdlValue::Bool(b) => b.to_string(),
        kdl::KdlValue::Null => String::new(),
    }
}

/// 从节点的子节点中获取具名字节点的字符串值。
fn get_child_string(node: &kdl::KdlNode, child_name: &str) -> Option<String> {
    node.children()
        .and_then(|doc| {
            doc.nodes()
                .iter()
                .find(|n| n.name().value() == child_name)
        })
        .and_then(|n| n.entries().first().map(|e| e.value()))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
}

/// 从节点的子节点中获取具名字节点的整数值。
fn get_child_integer(node: &kdl::KdlNode, child_name: &str) -> Option<i128> {
    node.children()
        .and_then(|doc| doc.nodes().iter().find(|n| n.name().value() == child_name))
        .and_then(|n| n.entries().first().map(|e| e.value()))
        .and_then(|v| v.as_integer())
}

/// 从节点的属性（键=值 entry）获取字符串值。
fn get_prop_string(node: &kdl::KdlNode, prop_name: &str) -> Option<String> {
    node.get(prop_name)
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
}

/// 从节点的属性（键=值 entry）获取布尔值。
fn get_prop_bool(node: &kdl::KdlNode, prop_name: &str) -> Option<bool> {
    node.get(prop_name).and_then(|v| v.as_bool())
}

/// 解析 opening 块到配置。
fn parse_opening_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    config.opening.exec = get_child_string(node, "exec");
    config.opening.file = get_child_string(node, "file");
}

/// 解析 prompts 块到配置。
fn parse_prompts_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(v) = get_child_string(node, "input_prompt_fg") {
        config.prompts.input_prompt_fg = v;
    }
    if let Some(v) = get_child_string(node, "input_prompt_format") {
        config.prompts.input_prompt_format = v;
    }
    if let Some(v) = get_child_string(node, "input_continue_format") {
        config.prompts.input_continue_format = v;
    }
    if let Some(v) = get_child_string(node, "output_prompt_format") {
        config.prompts.output_prompt_format = v;
    }
    if let Some(v) = get_child_string(node, "output_prompt_fg") {
        config.prompts.output_prompt_fg = v;
    }
    if let Some(v) = get_child_string(node, "bash_output_prompt_fg") {
        config.prompts.bash_output_prompt_fg = v;
    }
    if let Some(v) = get_child_string(node, "shell_type_fg") {
        config.prompts.shell_type_fg = v;
    }
}

/// 解析 NaCommands 块到配置。
fn parse_nacommands_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(children) = node.children() {
        for cmd_node in children.nodes() {
            let cmd_name = cmd_node.name().value().to_string();
            let exec = get_prop_string(cmd_node, "exec").unwrap_or_default();
            let long_argument =
                get_prop_bool(cmd_node, "long_argument").unwrap_or(false);
            let exec_script = get_prop_string(cmd_node, "exec_script");

            config.na_commands.insert(
                cmd_name,
                ExternalCmdConfig {
                    exec,
                    long_argument,
                    exec_script,
                },
            );
        }
    }
}

/// 解析 alias 块到配置。
fn parse_alias_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(children) = node.children() {
        for alias_node in children.nodes() {
            let alias_name = alias_node.name().value().to_string();
            if let Some(val) = alias_node.entries().first() {
                config
                    .aliases
                    .insert(alias_name, value_to_string(val.value()));
            }
        }
    }
}

/// 解析 shell 块到配置。
fn parse_shell_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(timeout) = get_child_integer(node, "timeout_secs") {
        config.shell.timeout_secs = timeout as u64;
    }
}

/// 解析 safety 块到配置。
fn parse_safety_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(children) = node.children() {
        for child_node in children.nodes() {
            if child_node.name().value() == "deny_patterns" {
                for entry in child_node.entries() {
                    config
                        .safety
                        .deny_patterns
                        .push(value_to_string(entry.value()));
                }
            }
        }
    }
}

/// 解析 plugins 块到配置。
fn parse_plugins_block(node: &kdl::KdlNode, config: &mut NashellConfig) {
    if let Some(dir) = get_child_string(node, "dir") {
        config.plugins.dir = dir;
    }
    if let Some(depth) = get_child_integer(node, "max_recursion_depth") {
        config.plugins.max_recursion_depth = depth as u32;
    }
}

/// 解析 KDL 格式的配置字符串。
///
/// 若输入为空字符串，返回默认配置。
/// 若解析失败，返回 `NashellError::Config`。
///
/// # 参数
/// - `content`: KDL 格式的配置文本
///
/// # 返回
/// `Result<NashellConfig, NashellError>` — 解析后的配置或错误
pub fn parse_kdl(content: &str) -> Result<NashellConfig, NashellError> {
    if content.trim().is_empty() {
        return Ok(NashellConfig::default());
    }

    let preprocessed = preprocess_bare_bools(content);

    let document = kdl::KdlDocument::parse(&preprocessed).map_err(|e| NashellError::Config {
        path: "<string>".to_string(),
        detail: format!("KDL parse error: {}", e),
    })?;

    let mut config = NashellConfig::default();

    for node in document.nodes() {
        let name = node.name().value().to_lowercase();

        match name.as_str() {
            "opening" => parse_opening_block(node, &mut config),
            "prompts" => parse_prompts_block(node, &mut config),
            "nacommands" => parse_nacommands_block(node, &mut config),
            "alias" => parse_alias_block(node, &mut config),
            "shell" => parse_shell_block(node, &mut config),
            "safety" => parse_safety_block(node, &mut config),
            "plugins" => parse_plugins_block(node, &mut config),
            _ => {}
        }
    }

    Ok(config)
}

/// 生成默认 KDL 配置内容。
///
/// 内容与 `NashellConfig::default()` 一致，格式为完整可编辑的 KDL 文件。
fn default_config_kdl() -> String {
    r#"// ===== NaShell 配置文件 =====
// 修改后重新启动生效。
// 删除此文件则下次启动自动重新生成默认配置。

// ===== 程序启动显示 =====
opening {
    // 执行命令 (如 "fastfetch")
    // exec ""
    // 或显示文件中的横幅
    // file ""
}

// ===== 提示符样式 =====
prompts {
    input_prompt_fg "green"
    input_prompt_format "{path} |> "
    input_continue_format ">> "
    output_prompt_format "@System #>>"
    output_prompt_fg "grey"
    bash_output_prompt_fg "bright_yellow"
    shell_type_fg "blue"
}

// ===== NaCommand 外部命令配置 =====
// NaCommands {
//     格式: <命令名> exec="<可执行文件>" long_argument=<true|false> [exec_script="<后缀>"]
//     example exec="nu ./example.nu" long_argument=false
// }

// ===== 命令别名 =====
// alias {
//     ll "ls -la"
// }

// ===== Shell 设置 =====
shell {
    timeout_secs 120
}

// ===== 安全设置 =====
safety {
    deny_patterns "rm -rf /" "rm -rf /*" "sudo rm -rf"
}

// ===== 插件设置 =====
plugins {
    dir "~/.config/nashell/plugins"
    max_recursion_depth 3
}
"#
    .to_string()
}

/// 在指定路径生成默认配置文件。
///
/// 自动创建父目录。若文件已存在则跳过。
fn write_default_config(path: &str) {
    // 创建父目录
    if let Some(parent) = std::path::Path::new(path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("Failed to create config directory '{}': {}", parent.display(), e);
            return;
        }
    }

    match std::fs::write(path, default_config_kdl()) {
        Ok(()) => {
            log::info!("Generated default config at '{}'", path);
        }
        Err(e) => {
            log::warn!("Failed to write default config to '{}': {}", path, e);
        }
    }
}

/// 从文件路径加载并解析 KDL 配置。
///
/// 加载优先级：
/// 1. 环境变量 `NASHELL_CONFIG` 指定的配置文件
/// 2. `~/.config/nashell/config.kdl`（默认路径）
/// 3. 内置默认值（文件不存在或无法解析时）
///
/// 配置文件不存在时不报错，使用默认值。
/// 若使用默认路径且文件不存在，自动生成一份带注释的默认配置文件。
/// 解析失败时报告错误但继续使用默认值。
///
/// # 参数
/// - `custom_path`: 可选的自定义配置文件路径（用于测试或 CLI 指定）
///
/// # 返回
/// `Result<NashellConfig, NashellError>` — 解析后的配置
pub fn load_config(custom_path: Option<&str>) -> Result<NashellConfig, NashellError> {
    let config_path = if let Some(path) = custom_path {
        path.to_string()
    } else if let Ok(env_path) = std::env::var("NASHELL_CONFIG") {
        env_path
    } else if let Some(home) = dirs::home_dir() {
        home.join(".config/nashell/config.kdl")
            .to_string_lossy()
            .to_string()
    } else {
        log::warn!("Cannot determine home directory, using defaults");
        return Ok(NashellConfig::default());
    };

    let config_dir = std::path::Path::new(&config_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("Config file not found at '{}', using defaults", config_path);
            // 仅在默认路径（非 env/custom）时生成配置文件
            if custom_path.is_none() && std::env::var("NASHELL_CONFIG").is_err() {
                write_default_config(&config_path);
            }
            return Ok(NashellConfig::default());
        }
        Err(e) => {
            log::warn!(
                "Failed to read config file '{}': {}, using defaults",
                config_path,
                e
            );
            return Ok(NashellConfig::default());
        }
    };

    match parse_kdl(&content) {
        Ok(mut config) => {
            config.config_dir = config_dir;
            log::info!("Config loaded successfully from '{}'", config_path);
            Ok(config)
        }
        Err(e) => {
            log::warn!(
                "Failed to parse config file '{}': {}, using defaults",
                config_path,
                e
            );
            Ok(NashellConfig::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_valid_kdl() {
        let kdl_content = r#"
            shell {
                timeout_secs 60
            }
        "#;
        let config = parse_kdl(kdl_content);
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.shell.timeout_secs, 60);
    }

    #[test]
    fn test_parse_empty_returns_defaults() {
        let config = parse_kdl("");
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.shell.timeout_secs, 120);
        assert_eq!(config.plugins.max_recursion_depth, 3);
    }

    #[test]
    fn test_parse_invalid_kdl_returns_error() {
        let bad_kdl = "this is not { valid kdl [";
        let result = parse_kdl(bad_kdl);
        assert!(result.is_err());
        match result {
            Err(NashellError::Config { .. }) => {}
            _ => panic!("expected Config error"),
        }
    }

    #[test]
    fn test_parse_prompts() {
        let kdl = r#"
            prompts {
                input_prompt_fg "blue"
                input_prompt_format "{path} $> "
                output_prompt_fg "red"
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.prompts.input_prompt_fg, "blue");
        assert_eq!(config.prompts.input_prompt_format, "{path} $> ");
        assert_eq!(config.prompts.output_prompt_fg, "red");
        assert_eq!(config.prompts.input_continue_format, ">> ");
    }

    #[test]
    fn test_parse_openings() {
        let kdl = r#"
            opening {
                exec "fastfetch"
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.opening.exec.as_deref(), Some("fastfetch"));
        assert!(config.opening.file.is_none());
    }

    #[test]
    fn test_parse_nacommands() {
        let kdl = r#"
            NaCommands {
                edit exec="n_edit" long_argument=true exec_script=".ned"
                websearch exec="nu ./web_search.nu" long_argument=false
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.na_commands.len(), 2);
        let edit = config.na_commands.get("edit").unwrap();
        assert_eq!(edit.exec, "n_edit");
        assert!(edit.long_argument);
        assert_eq!(edit.exec_script.as_deref(), Some(".ned"));
        let ws = config.na_commands.get("websearch").unwrap();
        assert_eq!(ws.exec, "nu ./web_search.nu");
        assert!(!ws.long_argument);
    }

    #[test]
    fn test_parse_aliases() {
        let kdl = r#"
            alias {
                ll "ls -la"
                gst "git status"
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.aliases.get("ll").map(|s| s.as_str()), Some("ls -la"));
        assert_eq!(
            config.aliases.get("gst").map(|s| s.as_str()),
            Some("git status")
        );
    }

    #[test]
    fn test_parse_safety() {
        let kdl = r#"
            safety {
                deny_patterns "rm -rf /" "sudo "
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.safety.deny_patterns.len(), 2);
        assert!(config.safety.deny_patterns.contains(&"rm -rf /".to_string()));
        assert!(config.safety.deny_patterns.contains(&"sudo ".to_string()));
    }

    #[test]
    fn test_parse_plugins() {
        let kdl = r#"
            plugins {
                dir "~/.config/nashell/plugins"
                max_recursion_depth 5
            }
        "#;
        let config = parse_kdl(kdl).unwrap();
        assert_eq!(config.plugins.dir, "~/.config/nashell/plugins");
        assert_eq!(config.plugins.max_recursion_depth, 5);
    }

    #[test]
    fn test_load_config_nonexistent_file() {
        let config = load_config(Some("/nonexistent/path/config.kdl"));
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.shell.timeout_secs, 120);
    }
}
