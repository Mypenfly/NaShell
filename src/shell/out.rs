/// Shell 线程的输出
#[derive(Debug, Clone)]
pub enum ShellOut {
    /// PTY 实时输出块
    Working(String),
    /// -c 模式捕获完毕
    Captured {
        /// 标准输出
        stdout: String,
        /// 标准错误输出
        stderr: String,
        /// 退出码
        exit_code: i32,
    },
    /// 命令执行完毕（PTY 模式）
    Wait,
    /// 确认已销毁
    Destroyed,
    /// 切换结果
    Switched {
        /// 新 shell 名称
        new_name: String,
        /// shell 唯一 id
        id: String,
    },
    /// 状态快照
    State {
        /// shell 名称
        name: String,
        /// shell 唯一 id
        id: String,
        /// 工作路径
        path: String,
        /// pools 中的条目计数
        pools_count: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working() {
        let out = ShellOut::Working("output chunk".to_string());
        match out {
            ShellOut::Working(s) => assert_eq!(s, "output chunk"),
            _ => panic!("Expected Working"),
        }
    }

    #[test]
    fn test_captured() {
        let out = ShellOut::Captured {
            stdout: "output".to_string(),
            stderr: "error".to_string(),
            exit_code: 0,
        };
        match out {
            ShellOut::Captured {
                stdout,
                stderr,
                exit_code,
            } => {
                assert_eq!(stdout, "output");
                assert_eq!(stderr, "error");
                assert_eq!(exit_code, 0);
            }
            _ => panic!("Expected Captured"),
        }
    }

    #[test]
    fn test_wait() {
        assert!(matches!(ShellOut::Wait, ShellOut::Wait));
    }

    #[test]
    fn test_destroyed() {
        assert!(matches!(ShellOut::Destroyed, ShellOut::Destroyed));
    }

    #[test]
    fn test_switched() {
        let out = ShellOut::Switched {
            new_name: "main".to_string(),
            id: "abc123".to_string(),
        };
        match out {
            ShellOut::Switched { new_name, id } => {
                assert_eq!(new_name, "main");
                assert_eq!(id, "abc123");
            }
            _ => panic!("Expected Switched"),
        }
    }

    #[test]
    fn test_state() {
        let out = ShellOut::State {
            name: "main".to_string(),
            id: "abc123".to_string(),
            path: "/home/user".to_string(),
            pools_count: 5,
        };
        match out {
            ShellOut::State {
                name,
                id,
                path,
                pools_count,
            } => {
                assert_eq!(name, "main");
                assert_eq!(id, "abc123");
                assert_eq!(path, "/home/user");
                assert_eq!(pools_count, 5);
            }
            _ => panic!("Expected State"),
        }
    }
}
