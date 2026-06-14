#[cfg(test)]
mod tests {
    // Tests will fail until types are defined below
    use super::*;

    #[test]
    fn test_cmd_type_shell() {
        let ct = CmdType::Shell;
        assert!(matches!(ct, CmdType::Shell));
    }

    #[test]
    fn test_cmd_type_nacommand_normal() {
        let ct = CmdType::NaCommandNormal;
        assert!(matches!(ct, CmdType::NaCommandNormal));
    }

    #[test]
    fn test_cmd_type_nacommand_system() {
        let ct = CmdType::NaCommandSystem;
        assert!(matches!(ct, CmdType::NaCommandSystem));
    }

    #[test]
    fn test_raw_cmd_construction() {
        let cmd = RawCmd {
            cmd_type: CmdType::Shell,
            cmd: "ls".to_string(),
            args: vec!["-la".to_string()],
        };
        assert_eq!(cmd.cmd, "ls");
        assert_eq!(cmd.args.len(), 1);
        assert_eq!(cmd.args[0], "-la");
        assert!(matches!(cmd.cmd_type, CmdType::Shell));
    }

    #[test]
    fn test_raw_cmd_with_nacommand_normal() {
        let cmd = RawCmd {
            cmd_type: CmdType::NaCommandNormal,
            cmd: "Write".to_string(),
            args: vec!["./test.txt".to_string()],
        };
        assert_eq!(cmd.cmd, "Write");
        assert!(matches!(cmd.cmd_type, CmdType::NaCommandNormal));
    }

    #[test]
    fn test_raw_commands_empty() {
        let cmds = RawCommands {
            commands: Vec::new(),
            long_argument: None,
            pre_out: None,
            async_name: None,
        };
        assert!(cmds.commands.is_empty());
        assert!(cmds.long_argument.is_none());
        assert!(cmds.pre_out.is_none());
        assert!(cmds.async_name.is_none());
    }

    #[test]
    fn test_raw_commands_with_content() {
        let cmds = RawCommands {
            commands: vec![RawCmd {
                cmd_type: CmdType::Shell,
                cmd: "echo".to_string(),
                args: vec!["hello".to_string()],
            }],
            long_argument: Some("print('hi')".to_string()),
            pre_out: Some("previous output".to_string()),
            async_name: Some("test".to_string()),
        };
        assert_eq!(cmds.commands.len(), 1);
        assert_eq!(cmds.long_argument.as_deref(), Some("print('hi')"));
        assert_eq!(cmds.pre_out.as_deref(), Some("previous output"));
        assert_eq!(cmds.async_name.as_deref(), Some("test"));
    }
}

/// 命令类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum CmdType {
    /// 普通 shell 命令（无特殊前缀）
    Shell,
    /// 普通 NaCommand（!@Cmd: 前缀）
    NaCommandNormal,
    /// 系统级 NaCommand（!!@Cmd: 前缀）
    NaCommandSystem,
}

/// 单个命令的解析结果
#[derive(Debug, Clone)]
pub struct RawCmd {
    /// 命令类型
    pub cmd_type: CmdType,
    /// 命令本体（如 "ls", "Write", "hx"）
    pub cmd: String,
    /// 命令行参数
    pub args: Vec<String>,
}

/// 解析后的命令集合
#[derive(Debug, Clone)]
pub struct RawCommands {
    /// 按管道分割后的命令列表
    pub commands: Vec<RawCmd>,
    /// @/ 或空行后的长参数内容
    pub long_argument: Option<String>,
    /// 前一个命令的管道输出（执行时填充）
    pub pre_out: Option<String>,
    /// 异步执行的目标 shell 名称，None 表示同步
    pub async_name: Option<String>,
}
