use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use crate::shell::cmd::ShellCmd;
use crate::shell::out::ShellOut;
use crate::shell::pty::PtySession;

/// 一个持久的 Shell 线程
///
/// 代表一个逻辑 Shell 环境。管理 shell 的状态和异步执行结果。
///
/// main shell（name="main"）不持有后台线程，其 path 与 Rust 进程 cwd 同步。
/// async shell 持有 `join_handle`，后台线程结束后为 None。
pub struct Shell {
    /// Shell 名称（"main" 为当前主 shell）
    pub name: String,
    /// 随机分配的唯一 id
    pub id: String,
    /// 工作路径（通过 PTY 同步）
    pub path: PathBuf,
    /// PTY 会话（当前架构下不持久化，保留供后续使用）
    pub pty: Option<PtySession>,
    /// 命令接收端
    pub cmd_rx: Receiver<ShellCmd>,
    /// 输出发送端
    pub out_tx: Sender<ShellOut>,
    /// 执行输出池（用于非 main shell 的异步执行结果积累）
    pub pools: Vec<String>,
    /// 后台线程句柄（仅 async shell 持有）
    pub join_handle: Option<JoinHandle<()>>,
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
            join_handle: None,
        };

        assert_eq!(shell.name, "main");
        assert_eq!(shell.id, "abc123");
        assert_eq!(shell.path, PathBuf::from("/home/user/projects"));
        assert!(shell.pools.is_empty());
        assert!(shell.pty.is_none());
        assert!(shell.join_handle.is_none());
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
            join_handle: None,
        };

        assert_eq!(shell.name, "async-test");
        assert_eq!(shell.pools.len(), 2);
        assert_eq!(shell.pools[0], "output 1");
    }
}
