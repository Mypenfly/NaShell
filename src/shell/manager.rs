use std::collections::HashMap;

use crate::error::NashellError;
use crate::shell::actor::Shell;
use crate::shell::pty::{spawn_pty_session, PtySession};

/// Shell 管理器，维护 main shell 和所有异步 shell。
///
/// 管理 PTY shell 的创建、销毁、命令发送和输出接收。
pub struct ShellManager {
    /// 主 shell 的 PTY 会话
    pub main_session: Option<PtySession>,
    /// 异步 shell 集合，按名称索引
    pub async_shells: HashMap<String, Shell>,
}

impl ShellManager {
    /// 创建空的管理器。
    pub fn new() -> Self {
        ShellManager {
            main_session: None,
            async_shells: HashMap::new(),
        }
    }

    /// 初始化主 shell PTY 会话。
    ///
    /// 创建 PTY 并启动指定的 shell 类型。
    ///
    /// # 参数
    /// - `shell_type`: shell 类型，如 `"bash"` 或 `"nu"`
    pub fn init_main_shell(&mut self, shell_type: &str) -> Result<(), NashellError> {
        let session = spawn_pty_session(shell_type)?;
        self.main_session = Some(session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_new() {
        let manager = ShellManager::new();
        assert!(manager.main_session.is_none());
        assert!(manager.async_shells.is_empty());
    }

    #[test]
    fn test_manager_init_main_shell() {
        let mut manager = ShellManager::new();
        let result = manager.init_main_shell("bash");
        assert!(result.is_ok(), "should init main shell with bash");
        assert!(manager.main_session.is_some());
    }

    #[test]
    fn test_manager_init_main_shell_invalid() {
        let mut manager = ShellManager::new();
        let result = manager.init_main_shell("nonexistent_shell");
        assert!(result.is_err());
    }
}
