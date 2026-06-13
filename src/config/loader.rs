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

    // Helper functions defined inside parse_kdl for convenience

    /// 将 KdlValue 转为 String
    fn value_to_string(val: &kdl::KdlValue) -> String {
        match val {
            kdl::KdlValue::String(s) => s.clone(),
            kdl::KdlValue::Integer(i) => i.to_string(),
            kdl::KdlValue::Float(f) => f.to_string(),
            kdl::KdlValue::Bool(b) => b.to_string(),
            kdl::KdlValue::Null => String::new(),
        }
    }

    /// 从节点的子节点中获取具名字节点的字符串值
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

    /// 从节点的子节点中获取具名字节点的整数值
    fn get_child_integer(node: &kdl::KdlNode, child_name: &str) -> Option<i128> {
        node.children()
            .and_then(|doc| doc.nodes().iter().find(|n| n.name().value() == child_name))
            .and_then(|n| n.entries().first().map(|e| e.value()))
            .and_then(|v| v.as_integer())
    }

    /// 从节点的属性（键=值 entry）获取字符串值
    fn get_prop_string(node: &kdl::KdlNode, prop_name: &str) -> Option<String> {
        node.get(prop_name)
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
    }

    /// 从节点的属性（键=值 entry）获取布尔值
    fn get_prop_bool(node: &kdl::KdlNode, prop_name: &str) -> Option<bool> {
        node.get(prop_name).and_then(|v| v.as_bool())
    }

    for node in document.nodes() {
        let name = node.name().value().to_lowercase();

        match name.as_str() {
            "opening" => {
                config.opening.exec = get_child_string(node, "exec");
                config.opening.file = get_child_string(node, "file");
            }
            "prompts" => {
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
            "nacommands" => {
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
            "alias" => {
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
            "shell" => {
                if let Some(timeout) = get_child_integer(node, "timeout_secs") {
                    config.shell.timeout_secs = timeout as u64;
                }
            }
            "safety" => {
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
            "plugins" => {
                if let Some(dir) = get_child_string(node, "dir") {
                    config.plugins.dir = dir;
                }
                if let Some(depth) = get_child_integer(node, "max_recursion_depth") {
                    config.plugins.max_recursion_depth = depth as u32;
                }
            }
            _ => {}
        }
    }

    Ok(config)
}

/// 从文件路径加载并解析 KDL 配置。
///
/// 加载优先级：
/// 1. 环境变量 `NASHELL_CONFIG` 指定的配置文件
/// 2. `~/.config/nashell/config.kdl`（默认路径）
/// 3. 内置默认值（文件不存在或无法解析时）
///
/// 配置文件不存在时不报错，使用默认值。
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

    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("Config file not found at '{}', using defaults", config_path);
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
        Ok(config) => {
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
