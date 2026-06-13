pub mod init;

/// 命令级别（与 NaLevel 对应但用于查表阶段）
#[derive(Debug, Clone, PartialEq)]
pub enum Level {
    /// 普通级别
    Normal,
    /// 系统级别
    System,
}

/// 命令元数据（内置/外部配置/插件共享）
#[derive(Debug, Clone)]
pub struct CmdMeta {
    /// 命令级别
    pub level: Level,
    /// 命令名称
    pub name: String,
    /// 执行程序路径
    pub exec: String,
    /// 是否接受 long_argument
    pub long_argument: bool,
    /// 可选执行脚本后缀
    pub exec_script: Option<String>,
}

/// 插件元数据
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// 插件名称
    pub name: String,
    /// 可执行文件路径
    pub exec: String,
    /// 是否订阅广播
    pub is_broadcast: bool,
    /// 注册的命令列表
    pub commands: Vec<CmdMeta>,
}

/// 程序运行时全局数据
#[derive(Debug, Clone, Default)]
pub struct AppData {
    /// 内置命令注册表
    pub builtin_cmds: Vec<CmdMeta>,
    /// 用户配置的外部 NaCommand
    pub config_cmds: Vec<CmdMeta>,
    /// 插件注册表
    pub plugins: Vec<PluginMeta>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_normal() {
        let level = Level::Normal;
        assert!(matches!(level, Level::Normal));
    }

    #[test]
    fn test_level_system() {
        let level = Level::System;
        assert!(matches!(level, Level::System));
    }

    #[test]
    fn test_cmd_meta_construction() {
        let meta = CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
        };
        assert_eq!(meta.name, "write");
        assert_eq!(meta.exec, "n_write");
        assert!(meta.long_argument);
        assert!(meta.exec_script.is_none());
    }

    #[test]
    fn test_cmd_meta_with_exec_script() {
        let meta = CmdMeta {
            level: Level::System,
            name: "config".to_string(),
            exec: "n_config".to_string(),
            long_argument: true,
            exec_script: Some(".conf".to_string()),
        };
        assert_eq!(meta.exec_script.as_deref(), Some(".conf"));
    }

    #[test]
    fn test_plugin_meta_construction() {
        let plugin = PluginMeta {
            name: "agent".to_string(),
            exec: "/path/to/agent".to_string(),
            is_broadcast: true,
            commands: Vec::new(),
        };
        assert_eq!(plugin.name, "agent");
        assert!(plugin.is_broadcast);
        assert!(plugin.commands.is_empty());
    }

    #[test]
    fn test_app_data_default() {
        let app = AppData::default();
        assert!(app.builtin_cmds.is_empty());
        assert!(app.config_cmds.is_empty());
        assert!(app.plugins.is_empty());
    }

    #[test]
    fn test_app_data_with_commands() {
        let app = AppData {
            builtin_cmds: vec![CmdMeta {
                level: Level::Normal,
                name: "write".to_string(),
                exec: "n_write".to_string(),
                long_argument: true,
                exec_script: None,
            }],
            config_cmds: Vec::new(),
            plugins: Vec::new(),
        };
        assert_eq!(app.builtin_cmds.len(), 1);
        assert_eq!(app.builtin_cmds[0].name, "write");
    }
}
