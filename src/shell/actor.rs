use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use crate::shell::cmd::ShellCmd;
use crate::shell::out::ShellOut;
use crate::shell::pty::PtySession;

/// 一个持久的 Shell 线程
///
/// 代表一个 PTY shell 会话。管理 shell 的输入输出通道和状态。
#[derive(Debug)]
pub struct Shell {
    /// Shell 名称（"main" 为当前主 shell）
    pub name: String,
    /// 随机分配的唯一 id
    pub id: String,
    /// 工作路径（通过 PTY 同步）
    pub path: PathBuf,
    /// PTY 会话
    pub pty: Option<PtySession>,
    /// 命令接收端
    pub cmd_rx: Receiver<ShellCmd>,
    /// 输出发送端
    pub out_tx: Sender<ShellOut>,
    /// 执行输出池（用于非 main shell 的异步执行结果积累）
    pub pools: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_shell_construction() {
        let (_cmd_tx, cmd_rx) = mpsc::channel::<ShellCmd>();
        let (out_tx, _out_rx) = mpsc::channel::<ShellOut>();

        let shell = Shell {
            name: "main".to_string(),
            id: "abc123".to_string(),
            path: PathBuf::from("/home/user/projects"),
            pty: None,
            cmd_rx,
            out_tx,
            pools: Vec::new(),
        };

        assert_eq!(shell.name, "main");
        assert_eq!(shell.id, "abc123");
        assert_eq!(shell.path, PathBuf::from("/home/user/projects"));
        assert!(shell.pools.is_empty());
        assert!(shell.pty.is_none());
    }

    #[test]
    fn test_shell_with_pools() {
        let (_cmd_tx, cmd_rx) = mpsc::channel::<ShellCmd>();
        let (out_tx, _out_rx) = mpsc::channel::<ShellOut>();

        let shell = Shell {
            name: "async-test".to_string(),
            id: "xyz789".to_string(),
            path: PathBuf::from("/tmp"),
            pty: None,
            cmd_rx,
            out_tx,
            pools: vec!["output 1".to_string(), "output 2".to_string()],
        };

        assert_eq!(shell.name, "async-test");
        assert_eq!(shell.pools.len(), 2);
        assert_eq!(shell.pools[0], "output 1");
    }
}
