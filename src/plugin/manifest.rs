use std::fs;
use std::path::{Path, PathBuf};

use crate::app::{CmdMeta, Level, PluginMeta};
use crate::error::NashellError;

/// 插件 manifest.json 的 JSON 结构——命令条目。
#[derive(Debug, serde::Deserialize)]
struct ManifestCommand {
    /// 命令级别
    level: String,
    /// 是否接受 long_argument
    long_argument: bool,
    /// 可选执行脚本后缀
    exec_script: Option<String>,
}

/// 插件 manifest.json 的 JSON 结构——顶层。
#[derive(Debug, serde::Deserialize)]
struct ManifestFile {
    /// 插件名称
    name: String,
    /// 可执行文件路径
    exec: String,
    /// 注册的命令
    nacommands: Option<std::collections::HashMap<String, ManifestCommand>>,
    /// 是否订阅广播
    is_broadcast: Option<bool>,
}

/// 从指定路径加载插件 manifest.json。
///
/// # 参数
/// - `path`: manifest.json 文件的完整路径
///
/// # 返回
/// 解析后的 PluginMeta
///
/// # 错误
/// - 文件不存在或不可读
/// - JSON 格式错误
/// - 必要字段缺失
pub fn load_manifest(path: &Path) -> Result<PluginMeta, NashellError> {
    let content = fs::read_to_string(path).map_err(|e| NashellError::Io {
        path: Some(path.display().to_string()),
        source: e,
    })?;

    let manifest: ManifestFile =
        serde_json::from_str(&content).map_err(|e| NashellError::Plugin {
            plugin_name: path.display().to_string(),
            detail: format!("manifest.json 解析失败: {}", e),
        })?;

    let commands: Vec<CmdMeta> = manifest
        .nacommands
        .unwrap_or_default()
        .into_iter()
        .map(|(cmd_name, cmd)| {
            let level = match cmd.level.to_lowercase().as_str() {
                "system" => Level::System,
                _ => Level::Normal,
            };
            CmdMeta {
                level,
                name: cmd_name.to_lowercase(),
                exec: manifest.exec.clone(),
                long_argument: cmd.long_argument,
                exec_script: cmd.exec_script,
                known_modes: Vec::new(),
            }
        })
        .collect();

    Ok(PluginMeta {
        name: manifest.name,
        exec: manifest.exec,
        is_broadcast: manifest.is_broadcast.unwrap_or(false),
        commands,
    })
}

/// 扫描插件目录，加载所有合法 manifest.json。
///
/// 遍历 `dir` 下的一级子目录，在每个子目录中查找 `manifest.json`，
/// 成功解析的插件收集到结果中，解析失败的插件记录警告日志后跳过。
///
/// # 参数
/// - `dir`: 插件根目录
///
/// # 返回
/// 成功加载的插件元数据列表
pub fn scan_plugins(dir: &Path) -> Vec<PluginMeta> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("无法读取插件目录 '{}': {}", dir.display(), e);
            return Vec::new();
        }
    };

    let mut plugins = Vec::new();

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let manifest_path = plugin_dir.join("manifest.json");
        if !manifest_path.is_file() {
            continue;
        }

        match load_manifest(&manifest_path) {
            Ok(meta) => {
                log::info!("加载插件: {} (exec={})", meta.name, meta.exec);
                plugins.push(meta);
            }
            Err(e) => {
                log::warn!("跳过无效的插件 manifest '{}': {}", manifest_path.display(), e);
            }
        }
    }

    plugins
}

/// 展开插件目录路径中的 `~` 符号。
///
/// 如果路径以 `~/` 开头，将其替换为实际 home 目录。
/// 否则返回原路径。
pub fn expand_plugins_dir(dir: &str) -> PathBuf {
    if dir.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&dir[2..]);
        }
    }
    PathBuf::from(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    static DIR_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn setup_temp_plugins_dir() -> PathBuf {
        let id = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nashell_plugin_manifest_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_manifest(dir: &Path, name: &str, content: &str) {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();
        let manifest_path = plugin_dir.join("manifest.json");
        let mut f = fs::File::create(&manifest_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_manifest_basic() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "test-plugin",
            "exec": "/usr/bin/test-plugin",
            "nacommands": {
                "agent": {
                    "level": "system",
                    "long_argument": true
                }
            },
            "is_broadcast": true
        }"#;
        write_manifest(&dir, "test-plugin", content);
        let manifest_path = dir.join("test-plugin").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert_eq!(meta.name, "test-plugin");
        assert_eq!(meta.exec, "/usr/bin/test-plugin");
        assert!(meta.is_broadcast);
        assert_eq!(meta.commands.len(), 1);
        assert_eq!(meta.commands[0].name, "agent");
        assert!(matches!(meta.commands[0].level, Level::System));
        assert!(meta.commands[0].long_argument);
    }

    #[test]
    fn test_load_manifest_multiple_commands() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "multi-cmd",
            "exec": "/usr/bin/multi",
            "nacommands": {
                "cmd1": {
                    "level": "normal",
                    "long_argument": false
                },
                "cmd2": {
                    "level": "system",
                    "long_argument": true,
                    "exec_script": ".conf"
                }
            },
            "is_broadcast": false
        }"#;
        write_manifest(&dir, "multi-cmd", content);
        let manifest_path = dir.join("multi-cmd").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert_eq!(meta.commands.len(), 2);
        assert!(!meta.is_broadcast);
        // HashMap iteration order is not guaranteed, check by name
        let names: Vec<&str> = meta.commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"cmd1"));
        assert!(names.contains(&"cmd2"));
        let cmd2 = meta.commands.iter().find(|c| c.name == "cmd2").unwrap();
        assert_eq!(cmd2.exec_script.as_deref(), Some(".conf"));
    }

    #[test]
    fn test_load_manifest_no_commands() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "no-cmds",
            "exec": "/usr/bin/no-cmds"
        }"#;
        write_manifest(&dir, "no-cmds", content);
        let manifest_path = dir.join("no-cmds").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert_eq!(meta.name, "no-cmds");
        assert!(meta.commands.is_empty());
    }

    #[test]
    fn test_load_manifest_is_broadcast_defaults_false() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "default-broadcast",
            "exec": "/usr/bin/default-bc"
        }"#;
        write_manifest(&dir, "default-broadcast", content);
        let manifest_path = dir.join("default-broadcast").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert!(!meta.is_broadcast);
    }

    #[test]
    fn test_load_manifest_level_normal_by_default() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "normal-level",
            "exec": "/usr/bin/normal",
            "nacommands": {
                "something": {
                    "level": "unknown",
                    "long_argument": false
                }
            }
        }"#;
        write_manifest(&dir, "normal-level", content);
        let manifest_path = dir.join("normal-level").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert!(matches!(meta.commands[0].level, Level::Normal));
    }

    #[test]
    fn test_load_manifest_invalid_json() {
        let dir = setup_temp_plugins_dir();
        let content = "{ invalid json }";
        write_manifest(&dir, "bad-json", content);
        let manifest_path = dir.join("bad-json").join("manifest.json");

        let result = load_manifest(&manifest_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_manifest_nonexistent() {
        let result = load_manifest(Path::new("/nonexistent/manifest.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_plugins_success() {
        let dir = setup_temp_plugins_dir();
        write_manifest(&dir, "plugin-a", r#"{"name": "plugin-a", "exec": "/usr/bin/a"}"#);
        write_manifest(&dir, "plugin-b", r#"{"name": "plugin-b", "exec": "/usr/bin/b", "is_broadcast": true}"#);

        let plugins = scan_plugins(&dir);
        assert_eq!(plugins.len(), 2);
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"plugin-a"));
        assert!(names.contains(&"plugin-b"));
    }

    #[test]
    fn test_scan_plugins_skips_invalid() {
        let dir = setup_temp_plugins_dir();
        write_manifest(&dir, "valid", r#"{"name": "valid", "exec": "/usr/bin/valid"}"#);
        write_manifest(&dir, "invalid", "{ bad json");

        let plugins = scan_plugins(&dir);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "valid");
    }

    #[test]
    fn test_scan_plugins_empty_dir() {
        let dir = setup_temp_plugins_dir();
        let plugins = scan_plugins(&dir);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_scan_plugins_nonexistent_dir() {
        let plugins = scan_plugins(Path::new("/nonexistent/plugins/dir"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_expand_plugins_dir_with_tilde() {
        let home = dirs::home_dir().unwrap();
        let result = expand_plugins_dir("~/my/plugins");
        assert_eq!(result, home.join("my/plugins"));
    }

    #[test]
    fn test_expand_plugins_dir_without_tilde() {
        let result = expand_plugins_dir("/absolute/path/plugins");
        assert_eq!(result, PathBuf::from("/absolute/path/plugins"));
    }

    #[test]
    fn test_command_name_is_lowercased() {
        let dir = setup_temp_plugins_dir();
        let content = r#"{
            "name": "mixed-case",
            "exec": "/usr/bin/mixed",
            "nacommands": {
                "MyCommand": {
                    "level": "normal",
                    "long_argument": false
                }
            }
        }"#;
        write_manifest(&dir, "mixed-case", content);
        let manifest_path = dir.join("mixed-case").join("manifest.json");

        let meta = load_manifest(&manifest_path).unwrap();
        assert_eq!(meta.commands[0].name, "mycommand");
    }
}
