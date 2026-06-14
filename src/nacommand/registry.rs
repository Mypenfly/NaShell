use crate::app::CmdMeta;
use crate::error::NashellError;

/// 命令注册表，管理所有 NaCommand 的注册、查表和帮助信息。
///
/// 查表优先级：内置命令 → 配置命令 → 插件命令（后两者在后续 Phase 完善）。
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    /// 内置命令列表（Write、Open 等）
    pub builtin_cmds: Vec<CmdMeta>,
}

impl CommandRegistry {
    /// 创建空的命令注册表。
    pub fn new() -> Self {
        CommandRegistry {
            builtin_cmds: Vec::new(),
        }
    }

    /// 注册单个内置命令。
    ///
    /// # 参数
    /// - `meta`: 命令元数据
    pub fn register_builtin(&mut self, meta: CmdMeta) {
        self.builtin_cmds.push(meta);
    }

    /// 查表查找命令元数据。
    ///
    /// 按小写匹配，依次查找内置命令、配置命令、插件命令。
    /// 当前仅支持内置命令查表。
    ///
    /// # 参数
    /// - `cmd_name`: 命令名称（大小写不敏感）
    ///
    /// # 返回
    /// - `Ok(&CmdMeta)`: 找到的命令元数据
    /// - `Err(NashellError::CommandNotFound)`: 未找到
    pub fn lookup(&self, cmd_name: &str) -> Result<&CmdMeta, NashellError> {
        let lower_name = cmd_name.to_lowercase();

        for cmd in &self.builtin_cmds {
            if cmd.name.to_lowercase() == lower_name {
                return Ok(cmd);
            }
        }

        // TODO: Phase 9+ 查 config_cmds, Phase 8+ 查 plugins[*].commands

        Err(NashellError::CommandNotFound {
            name: cmd_name.to_string(),
        })
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

        let builtin_helps: std::collections::HashMap<&str, &str> = {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "write",
                "Write\n  将内容写入文件。\n\n  参数:\n    path    目标文件路径（必须，命令名后的第一个参数）\n    content 要写入的内容，来自 long_argument（非必须）\n\n  使用示例:\n    !@Write:./example.py @/\n      > x = int(input())\n      > print(x * 18)\n\n  注意: long_argument 为 None 时创建空文件或清空既有文件。",
            );
            m.insert(
                "open",
                "Open\n  打开文件或文件夹。\n\n  参数:\n    path           目标路径（必须）\n    --limit/-l     限制行数（默认 500，仅文件有效）\n    --start/-s     起始行（默认 1，仅文件有效）\n    --end/-e       结束行（仅文件有效）\n\n  使用示例:\n    !@Open:./src           显示目录结构树\n    !@Open:./test.py -l 50 显示文件前 50 行（带行号）\n\n  注意: 目标为目录时不可传入 --limit/--start/--end，否则报错。",
            );
            m
        };

        for builtin_help in &self.builtin_cmds {
            if builtin_help.name.to_lowercase() == lower_name {
                if let Some(help_text) = builtin_helps.get(lower_name.as_str()) {
                    return Ok(help_text.to_string());
                }
                break;
            }
        }

        // TODO: Phase 9+ 外部命令 help 透传 --help
        // TODO: Phase 8+ 插件命令 help call

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
}
