/// 命令级别
#[derive(Debug, Clone, PartialEq)]
pub enum NaLevel {
    /// 普通级别命令（!@NaCommand:）
    Normal,
    /// 系统级别命令（!!@NaCommand:）
    System,
}

/// NaCommand 执行时的数据结构
#[derive(Debug, Clone)]
pub struct NaCommand {
    /// 命令级别
    pub level: NaLevel,
    /// 命令名（小写）
    pub cmd: String,
    /// 子命令/模式（如 "watch", "help"），None 表示默认模式
    pub mode: Option<String>,
    /// 选项参数（如 ["-q", "rust", "-c", "10"]）
    pub args: Vec<String>,
    /// 多行长参数
    pub long_argument: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nalevel_normal() {
        let level = NaLevel::Normal;
        assert!(matches!(level, NaLevel::Normal));
    }

    #[test]
    fn test_nalevel_system() {
        let level = NaLevel::System;
        assert!(matches!(level, NaLevel::System));
    }

    #[test]
    fn test_nacommand_default_construction() {
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: Vec::new(),
            long_argument: None,
        };
        assert_eq!(cmd.cmd, "write");
        assert!(cmd.mode.is_none());
        assert!(cmd.args.is_empty());
        assert!(cmd.long_argument.is_none());
    }

    #[test]
    fn test_nacommand_with_mode() {
        let cmd = NaCommand {
            level: NaLevel::System,
            cmd: "shell".to_string(),
            mode: Some("watch".to_string()),
            args: vec!["-i".to_string(), "abc".to_string(), "-c".to_string(), "3".to_string()],
            long_argument: None,
        };
        assert_eq!(cmd.cmd, "shell");
        assert_eq!(cmd.mode.as_deref(), Some("watch"));
        assert_eq!(cmd.args.len(), 4);
    }

    #[test]
    fn test_nacommand_with_long_argument() {
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "write".to_string(),
            mode: None,
            args: vec!["./test.py".to_string()],
            long_argument: Some("print('hello')".to_string()),
        };
        assert_eq!(cmd.args[0], "./test.py");
        assert_eq!(cmd.long_argument.as_deref(), Some("print('hello')"));
    }
}
