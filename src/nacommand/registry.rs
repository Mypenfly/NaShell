use crate::app::{CmdMeta, PluginMeta};
use crate::error::NashellError;

/// 命令注册表，管理所有 NaCommand 的注册、查表和帮助信息。
///
/// 查表优先级：内置命令 → 配置命令 → 插件命令。
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    /// 内置命令列表（Write、Open 等）
    pub builtin_cmds: Vec<CmdMeta>,
    /// 用户配置的外部命令列表
    pub config_cmds: Vec<CmdMeta>,
    /// 插件注册的命令列表
    pub plugin_cmds: Vec<CmdMeta>,
    /// 命令名 → 插件名的映射（仅插件命令）
    pub cmd_to_plugin: std::collections::HashMap<String, String>,
}

/// ANSI 颜色样式常量，用于帮助文本格式化。
mod style {
    pub const BOLD: &str = "\x1b[1m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const GREEN: &str = "\x1b[32m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const RESET: &str = "\x1b[0m";
}

/// 为文本包裹加亮青色（命令名）。
fn style_cmd_name(text: &str) -> String {
    format!("{}{}{}{}", style::BOLD, style::BRIGHT_CYAN, text, style::RESET)
}

/// 为文本包裹亮蓝色（段落标题）。
fn style_section(title: &str) -> String {
    format!("{}{}{}", style::BRIGHT_BLUE, title, style::RESET)
}

/// 为代码示例包裹绿色。
fn style_code(code: &str) -> String {
    format!("{}{}{}", style::GREEN, code, style::RESET)
}

/// 为注意事项包裹亮黄色。
fn style_note(text: &str) -> String {
    format!("{}{}{}", style::BRIGHT_YELLOW, text, style::RESET)
}

/// 查表结果，标识命令来源。
#[derive(Debug, Clone, PartialEq)]
pub enum LookupSource {
    /// 内置命令
    Builtin,
    /// 用户配置的外部命令
    Config,
    /// 插件命令
    Plugin,
}

impl CommandRegistry {
    /// 创建空的命令注册表。
    pub fn new() -> Self {
        CommandRegistry {
            builtin_cmds: Vec::new(),
            config_cmds: Vec::new(),
            plugin_cmds: Vec::new(),
            cmd_to_plugin: std::collections::HashMap::new(),
        }
    }

    /// 注册单个内置命令。
    ///
    /// # 参数
    /// - `meta`: 命令元数据
    pub fn register_builtin(&mut self, meta: CmdMeta) {
        self.builtin_cmds.push(meta);
    }

    /// 加载插件命令到注册表中。
    ///
    /// 从插件元数据列表中提取所有注册的命令，统一存入 plugin_cmds，
    /// 并建立命令名到插件名的映射。
    ///
    /// # 参数
    /// - `plugins`: 插件元数据列表
    pub fn load_plugins(&mut self, plugins: &[PluginMeta]) {
        self.plugin_cmds.clear();
        self.cmd_to_plugin.clear();
        for plugin in plugins {
            for cmd in &plugin.commands {
                let cmd_name = cmd.name.to_lowercase();
                self.cmd_to_plugin.insert(cmd_name, plugin.name.clone());
                self.plugin_cmds.push(cmd.clone());
            }
        }
    }

    /// 查询指定命令所属的插件名称。
    ///
    /// # 参数
    /// - `cmd_name`: 命令名称（大小写不敏感）
    ///
    /// # 返回
    /// 插件名称（如果命令属于某个插件），否则为 None
    pub fn command_owner(&self, cmd_name: &str) -> Option<&String> {
        self.cmd_to_plugin.get(&cmd_name.to_lowercase())
    }

    /// 查表查找命令元数据，同时返回来源。
    ///
    /// 按小写匹配，依次查找内置命令、配置命令、插件命令。
    ///
    /// # 参数
    /// - `cmd_name`: 命令名称（大小写不敏感）
    ///
    /// # 返回
    /// - `Ok((&CmdMeta, LookupSource))`: 找到的命令元数据及来源
    /// - `Err(NashellError::CommandNotFound)`: 未找到
    pub fn lookup_with_source(&self, cmd_name: &str) -> Result<(&CmdMeta, LookupSource), NashellError> {
        let lower_name = cmd_name.to_lowercase();

        for cmd in &self.builtin_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok((cmd, LookupSource::Builtin));
            }
        }

        for cmd in &self.config_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok((cmd, LookupSource::Config));
            }
        }

        for cmd in &self.plugin_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok((cmd, LookupSource::Plugin));
            }
        }

        Err(NashellError::CommandNotFound {
            name: cmd_name.to_string(),
        })
    }

    /// 查表查找命令元数据。
    ///
    /// 按小写匹配，依次查找内置命令、配置命令、插件命令。
    ///
    /// # 参数
    /// - `cmd_name`: 命令名称（大小写不敏感）
    ///
    /// # 返回
    /// - `Ok(&CmdMeta)`: 找到的命令元数据
    /// - `Err(NashellError::CommandNotFound)`: 未找到
    pub fn lookup(&self, cmd_name: &str) -> Result<&CmdMeta, NashellError> {
        self.lookup_with_source(cmd_name).map(|(meta, _)| meta)
    }

    /// 获取命令的帮助信息。
    ///
    /// 对内置命令返回内置帮助文本，对配置命令传 `--help` 透传输出，
    /// 对插件命令发送 call 消息获取帮助。
    ///
    /// # 参数
    /// - `cmd_name`: 命令名称（大小写不敏感）
    /// - `mode`: 子命令/模式
    ///
    /// # 返回
    /// - `Ok(String)`: 帮助文本
    /// - `Err(NashellError::CommandNotFound)`: 命令未找到
    pub fn get_help(&self, cmd_name: &str, _mode: Option<&str>) -> Result<String, NashellError> {
        let lower_name = cmd_name.to_lowercase();

        let builtin_helps: std::collections::HashMap<String, String> = {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "write".to_string(),
                format!(
                    "{}\n  将内容写入文件。\n\n  {}  \
                     path    目标文件路径（必须，命令名后的第一个参数）\n    \
                     content 要写入的内容，来自 long_argument（非必须）\n\n  {}  \
                     \n    {}\n      {}  \
                     \n\n  {} long_argument 为 None 时创建空文件或清空既有文件。",
                    style_cmd_name("Write"),
                    style_section("参数:"),
                    style_section("使用示例:"),
                    style_code("!@Write:./example.py @/"),
                    style_code("> x = int(input())\n      > print(x * 18)"),
                    style_note("注意:")
                ),
            );
            m.insert(
                "open".to_string(),
                format!(
                    "{}\n  打开文件或文件夹。\n\n  {}  \
                     path           目标路径（必须）\n    \
                     --limit/-l     限制行数（文件，默认 500）/ 递归深度（目录，默认 3）\n    \
                     --start/-s     起始行（默认 1，仅文件有效）\n    \
                     --end/-e       结束行（仅文件有效）\n\n  {}  \
                     \n    {}\n    {}\n\n  {} \
                     目标为目录时不可传入 --start/--end，否则报错。\n  \
                     {} 文件内容支持语法高亮。",
                    style_cmd_name("Open"),
                    style_section("参数:"),
                    style_section("使用示例:"),
                    style_code("!@Open:./src           显示目录结构树（深度 3）"),
                    style_code("!@Open:./test.py -l 50 显示文件前 50 行（带行号+语法高亮）"),
                    style_note("注意:"),
                    style_note("注意:")
                ),
            );
            m.insert(
                "bash".to_string(),
                format!(
                    "{}\n  使用 bash 执行命令。\n\n  {}  \
                     !!@Bash: 的优先级最高，跳过所有其他解析规则。\n  \
                     支持 @/Async(name) 异步执行（创建临时 bash 子进程）。\n\n  {}  \
                     \n    {}\n\n  {}  \
                     输出使用亮黄色 Bash: 标识。",
                    style_cmd_name("Bash"),
                    style_section("特点:"),
                    style_section("使用示例:"),
                    style_code("!!@Bash: ls -la"),
                    style_note("注意:")
                ),
            );
            m.insert(
                "shell".to_string(),
                format!(
                    "{}\n  管理 NaShell 内部的 Shell 线程。\n\n  {}  \
                     \n    Shell:           列出所有 Shell 状态\n    \
                     Shell:Watch      查看指定 shell 的 pools（-i id [-c count]）\n    \
                     Shell:Destroy    销毁指定 shell（-i id）\n    \
                     Shell:Switch     切换 main shell（-i id [-d]）\n\n  {}  \
                     \n    {}\n    {}\n    {}\n    {}\n\n  {}  \
                     Shell 命令通过 id 定位目标 shell，id 由系统自动分配。",
                    style_cmd_name("Shell"),
                    style_section("模式:"),
                    style_section("使用示例:"),
                    style_code("!!@Shell:"),
                    style_code("!!@Shell:Watch -i abc123 -c 3"),
                    style_code("!!@Shell:Destroy -i abc123"),
                    style_code("!!@Shell:Switch -i abc123 -d"),
                    style_note("注意:")
                ),
            );
            m.insert(
                "nacmds".to_string(),
                format!(
                    "{}\n  列出所有已注册的 NaCommand（内置 / 用户配置 / 插件）。\n\n  {}  \
                     \n    NaCmds:          默认模式，表格输出命令名、级别、来源\n    \
                     NaCmds:Detail     详细模式，包含帮助信息\n    \
                     NaCmds:Help       显示此帮助\n\n  {}  \
                     \n    -j / --json      以 JSON 格式输出\n\n  {}  \
                     \n    {}\n    {}",
                    style_cmd_name("NaCmds"),
                    style_section("模式:"),
                    style_section("选项:"),
                    style_section("使用示例:"),
                    style_code("!@NaCmds:"),
                    style_code("!@NaCmds:Detail -j"),
                ),
            );
            m
        };

        // Check builtin commands first
        for builtin_help in &self.builtin_cmds {
            if builtin_help.name.to_lowercase() == lower_name {
                if let Some(help_text) = builtin_helps.get(&lower_name) {
                    return Ok(help_text.clone());
                }
                return Ok(format!("{} 帮助信息暂未提供", style_cmd_name(cmd_name)));
            }
        }

        // For config commands, return a generic message
        for cmd in &self.config_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok(format!(
                    "{} (用户配置命令)\n执行外部程序: {}\n使用 Help 模式获取该程序的帮助信息。",
                    style_cmd_name(cmd_name),
                    cmd.exec,
                ));
            }
        }

        // For plugin commands, return a generic message
        // (Actual help is obtained by sending a call message with mode="help")
        for cmd in &self.plugin_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok(format!(
                    "{} (插件命令)\n请使用 Help 模式获取详细帮助信息。",
                    style_cmd_name(cmd_name)
                ));
            }
        }

        Err(NashellError::CommandNotFound {
            name: cmd_name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CmdMeta, Level};

    #[test]
    fn test_registry_new_is_empty() {
        let registry = CommandRegistry::new();
        assert!(registry.builtin_cmds.is_empty());
        assert!(registry.config_cmds.is_empty());
        assert!(registry.plugin_cmds.is_empty());
    }

    #[test]
    fn test_register_builtin_lookup() {
        let mut registry = CommandRegistry::new();
        let cmd_meta = CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        };
        registry.builtin_cmds.push(cmd_meta);

        let found = registry.lookup("write").unwrap();
        assert_eq!(found.name, "write");
        assert_eq!(found.exec, "n_write");
        assert!(found.long_argument);
        assert_eq!(found.known_modes, vec!["help"]);
    }

    #[test]
    fn test_lookup_case_insensitive() {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });

        assert!(registry.lookup("Write").is_ok());
        assert!(registry.lookup("WRITE").is_ok());
        assert!(registry.lookup("write").is_ok());
    }

    #[test]
    fn test_lookup_not_found() {
        let registry = CommandRegistry::new();
        let result = registry.lookup("nonexistent");
        assert!(result.is_err());
        match result {
            Err(crate::error::NashellError::CommandNotFound { name }) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("expected CommandNotFound error"),
        }
    }

    #[test]
    fn test_get_help_builtin() {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });

        let help = registry.get_help("write", None).unwrap();
        assert!(help.contains("Write"));
        assert!(help.contains("写入文件"));
    }

    #[test]
    fn test_get_help_nonexistent() {
        let registry = CommandRegistry::new();
        let result = registry.get_help("nonexistent", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_builtin_registers_multiple() {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "open".to_string(),
            exec: "n_open".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });

        assert_eq!(registry.builtin_cmds.len(), 2);
        assert!(registry.lookup("write").is_ok());
        assert!(registry.lookup("open").is_ok());
    }

    #[test]
    fn test_lookup_with_source_builtin() {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });

        let (meta, source) = registry.lookup_with_source("write").unwrap();
        assert_eq!(meta.name, "write");
        assert_eq!(source, LookupSource::Builtin);
    }

    #[test]
    fn test_lookup_with_source_plugin() {
        let mut registry = CommandRegistry::new();
        registry.plugin_cmds.push(CmdMeta {
            level: Level::System,
            name: "agent".to_string(),
            exec: "/usr/bin/agent".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });

        let (meta, source) = registry.lookup_with_source("agent").unwrap();
        assert_eq!(meta.name, "agent");
        assert_eq!(source, LookupSource::Plugin);
    }

    #[test]
    fn test_get_help_plugin_command() {
        let mut registry = CommandRegistry::new();
        registry.plugin_cmds.push(CmdMeta {
            level: Level::System,
            name: "agent".to_string(),
            exec: "/usr/bin/agent".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });

        let help = registry.get_help("agent", None).unwrap();
        assert!(help.contains("agent"));
        assert!(help.contains("插件命令"));
    }

    #[test]
    fn test_lookup_priority_builtin_over_plugin() {
        let mut registry = CommandRegistry::new();
        registry.builtin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });
        registry.plugin_cmds.push(CmdMeta {
            level: Level::System,
            name: "write".to_string(),
            exec: "/usr/bin/plugin-write".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });

        let (meta, source) = registry.lookup_with_source("write").unwrap();
        assert_eq!(meta.exec, "n_write");
        assert_eq!(source, LookupSource::Builtin);
    }

    #[test]
    fn test_load_plugins() {
        let mut registry = CommandRegistry::new();
        let plugins = vec![PluginMeta {
            name: "test-plugin".to_string(),
            exec: "/usr/bin/test".to_string(),
            is_broadcast: false,
            commands: vec![CmdMeta {
                level: Level::Normal,
                name: "cmd1".to_string(),
                exec: "/usr/bin/test".to_string(),
                long_argument: true,
                exec_script: None,
                known_modes: vec![],
            }],
        }];

        registry.load_plugins(&plugins);
        assert_eq!(registry.plugin_cmds.len(), 1);
        assert!(registry.lookup("cmd1").is_ok());
    }

    #[test]
    fn test_lookup_with_source_config() {
        let mut registry = CommandRegistry::new();
        registry.config_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "websearch".to_string(),
            exec: "nu ./web_search.nu".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });

        let (meta, source) = registry.lookup_with_source("websearch").unwrap();
        assert_eq!(meta.name, "websearch");
        assert_eq!(source, LookupSource::Config);
        assert_eq!(meta.exec, "nu ./web_search.nu");
    }

    #[test]
    fn test_get_help_config_command() {
        let mut registry = CommandRegistry::new();
        registry.config_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "websearch".to_string(),
            exec: "nu ./web_search.nu".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });

        let help = registry.get_help("websearch", None).unwrap();
        assert!(help.contains("websearch"));
        assert!(help.contains("用户配置命令"));
        assert!(help.contains("nu ./web_search.nu"));
    }

    #[test]
    fn test_lookup_priority_config_over_plugin() {
        let mut registry = CommandRegistry::new();
        registry.config_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "mycmd".to_string(),
            exec: "my_config_exec".to_string(),
            long_argument: false,
            exec_script: None,
            known_modes: vec![],
        });
        registry.plugin_cmds.push(CmdMeta {
            level: Level::Normal,
            name: "mycmd".to_string(),
            exec: "my_plugin_exec".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec![],
        });

        let (meta, source) = registry.lookup_with_source("mycmd").unwrap();
        assert_eq!(meta.exec, "my_config_exec");
        assert_eq!(source, LookupSource::Config);
    }
}
