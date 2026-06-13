/// 发给 Shell 线程的命令
#[derive(Debug, Clone)]
pub enum ShellCmd {
    /// 在 PTY 中直接执行（实时输出）
    ExecPty {
        /// 输入的命令文本
        input: String,
    },
    /// 通过 -c 捕获执行
    ExecCaptured {
        /// 执行的命令
        cmd: String,
        /// 命令参数
        args: Vec<String>,
    },
    /// 切换为 main shell
    Switch(String),
    /// 中断当前执行
    Stop,
    /// 销毁线程
    Destroy,
    /// 查看 pools 中最近 count 条输出
    Watch {
        /// 查看数量
        count: usize,
    },
    /// 获取状态快照
    GetState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_pty() {
        let cmd = ShellCmd::ExecPty {
            input: "ls -la".to_string(),
        };
        match cmd {
            ShellCmd::ExecPty { input } => assert_eq!(input, "ls -la"),
            _ => panic!("Expected ExecPty"),
        }
    }

    #[test]
    fn test_exec_captured() {
        let cmd = ShellCmd::ExecCaptured {
            cmd: "ls".to_string(),
            args: vec!["-la".to_string()],
        };
        match cmd {
            ShellCmd::ExecCaptured { cmd, args } => {
                assert_eq!(cmd, "ls");
                assert_eq!(args, vec!["-la"]);
            }
            _ => panic!("Expected ExecCaptured"),
        }
    }

    #[test]
    fn test_switch() {
        let cmd = ShellCmd::Switch("test-shell".to_string());
        match cmd {
            ShellCmd::Switch(name) => assert_eq!(name, "test-shell"),
            _ => panic!("Expected Switch"),
        }
    }

    #[test]
    fn test_stop() {
        assert!(matches!(ShellCmd::Stop, ShellCmd::Stop));
    }

    #[test]
    fn test_destroy() {
        assert!(matches!(ShellCmd::Destroy, ShellCmd::Destroy));
    }

    #[test]
    fn test_watch() {
        let cmd = ShellCmd::Watch { count: 3 };
        match cmd {
            ShellCmd::Watch { count } => assert_eq!(count, 3),
            _ => panic!("Expected Watch"),
        }
    }

    #[test]
    fn test_get_state() {
        assert!(matches!(ShellCmd::GetState, ShellCmd::GetState));
    }
}
