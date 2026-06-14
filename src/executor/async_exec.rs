use std::sync::{Arc, Mutex};

use crate::error::NashellError;
use crate::executor::shell_exec;
use crate::shell::manager::ShellManager;

/// 异步执行结果信息。
#[derive(Debug, Clone)]
pub struct AsyncExecInfo {
    /// Shell 名称
    pub name: String,
    /// Shell 唯一 id
    pub id: String,
}

/// 启动异步 Shell 命令执行。
///
/// 在后台线程中运行 captured 模式执行，结果存入指定 shell 的 pools。
/// 若同名 shell 已存在则复用，否则创建新 shell。
/// 线程创建后立即返回 shell 信息。
///
/// # 参数
/// - `name`: 异步 shell 的名称（来自 @/Async(name)）
/// - `command`: 要执行的命令字符串（@/Async 之前的部分）
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
/// - `timeout_secs`: 命令超时秒数
/// - `manager`: Shell 管理器（共享引用）
/// - `cwd`: 当前工作目录快照
pub fn spawn_async_shell_exec(
    name: &str,
    command: &str,
    shell_type: &str,
    timeout_secs: u64,
    manager: &Arc<Mutex<ShellManager>>,
    cwd: &std::path::PathBuf,
) -> Result<AsyncExecInfo, NashellError> {
    let (shell_id, shell_name) = {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: command.to_string(),
            exit_code: None,
            stderr: format!("获取 ShellManager 锁失败: {}", e),
        })?;
        let id = mgr.get_or_create_shell(name, cwd);
        let sname = mgr
            .get_shell_name(&id)
            .unwrap_or("unknown")
            .to_string();
        (id, sname)
    };

    // 克隆需要移动到线程中的数据
    let command_owned = command.to_string();
    let shell_type_owned = shell_type.to_string();
    let manager_clone = Arc::clone(manager);
    let shell_id_clone = shell_id.clone();

    let handle = std::thread::spawn(move || {
        let result = shell_exec::exec_captured(
            &command_owned,
            &[],
            &shell_type_owned,
            timeout_secs,
        );

        let output = match result {
            Ok(captured) => {
                let mut msg = captured.stdout;
                if !captured.stderr.is_empty() {
                    if !msg.is_empty() {
                        msg.push('\n');
                    }
                    msg.push_str(&captured.stderr);
                }
                msg
            }
            Err(e) => {
                format!("@Error #>>\n{}", e)
            }
        };

        if let Ok(mut mgr) = manager_clone.lock() {
            let _ = mgr.add_to_pools(&shell_id_clone, &output);
        }
    });

    // 保存线程句柄
    {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: command.to_string(),
            exit_code: None,
            stderr: format!("获取 ShellManager 锁失败: {}", e),
        })?;
        mgr.set_join_handle(&shell_id, handle)?;
    }

    Ok(AsyncExecInfo {
        name: shell_name,
        id: shell_id,
    })
}

/// 启动异步 Bash 命令执行。
///
/// 在后台线程中运行 `exec_bash`，结果存入指定 shell 的 pools。
/// 使用当前 main shell 的工作目录作为 shell 的 path。
///
/// # 参数
/// - `name`: 异步 shell 的名称
/// - `bash_args`: 传给 `bash -c` 的参数字符串
/// - `timeout_secs`: 命令超时秒数
/// - `manager`: Shell 管理器（共享引用）
/// - `cwd`: 当前工作目录快照
pub fn spawn_async_bash_exec(
    name: &str,
    bash_args: &str,
    timeout_secs: u64,
    manager: &Arc<Mutex<ShellManager>>,
    cwd: &std::path::PathBuf,
) -> Result<AsyncExecInfo, NashellError> {
    let (shell_id, shell_name) = {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: bash_args.to_string(),
            exit_code: None,
            stderr: format!("获取 ShellManager 锁失败: {}", e),
        })?;
        let id = mgr.get_or_create_shell(name, cwd);
        let sname = mgr
            .get_shell_name(&id)
            .unwrap_or("unknown")
            .to_string();
        (id, sname)
    };

    let bash_args_owned = bash_args.to_string();
    let manager_clone = Arc::clone(manager);
    let shell_id_clone = shell_id.clone();

    let handle = std::thread::spawn(move || {
        let result = shell_exec::exec_bash(&bash_args_owned, timeout_secs);

        let output = match result {
            Ok(captured) => {
                let mut msg = captured.stdout;
                if !captured.stderr.is_empty() {
                    if !msg.is_empty() {
                        msg.push('\n');
                    }
                    msg.push_str(&captured.stderr);
                }
                msg
            }
            Err(e) => {
                format!("@Error #>>\n{}", e)
            }
        };

        if let Ok(mut mgr) = manager_clone.lock() {
            let _ = mgr.add_to_pools(&shell_id_clone, &output);
        }
    });

    {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: bash_args.to_string(),
            exit_code: None,
            stderr: format!("获取 ShellManager 锁失败: {}", e),
        })?;
        mgr.set_join_handle(&shell_id, handle)?;
    }

    Ok(AsyncExecInfo {
        name: shell_name,
        id: shell_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_manager() -> Arc<Mutex<ShellManager>> {
        let mut mgr = ShellManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.register_main(&cwd);
        Arc::new(Mutex::new(mgr))
    }

    #[test]
    fn test_spawn_async_shell_exec() {
        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();

        let info = spawn_async_shell_exec(
            "test_async",
            "echo hello_async_shell",
            "bash",
            120,
            &mgr,
            &cwd,
        )
        .unwrap();

        assert_eq!(info.name, "test_async");
        assert!(!info.id.is_empty());

        // 验证 shell 已创建
        let m = mgr.lock().unwrap();
        assert!(m.shells.contains_key(&info.id));
    }

    #[test]
    fn test_spawn_async_bash_exec() {
        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();

        let info = spawn_async_bash_exec(
            "bash_test",
            "echo hello_async_bash",
            120,
            &mgr,
            &cwd,
        )
        .unwrap();

        assert_eq!(info.name, "bash_test");
        assert!(!info.id.is_empty());

        let m = mgr.lock().unwrap();
        assert!(m.shells.contains_key(&info.id));
    }

    #[test]
    fn test_spawn_async_shell_reuse() {
        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();

        let info1 = spawn_async_shell_exec(
            "reuse_test",
            "echo first",
            "bash",
            120,
            &mgr,
            &cwd,
        )
        .unwrap();

        let info2 = spawn_async_shell_exec(
            "reuse_test",
            "echo second",
            "bash",
            120,
            &mgr,
            &cwd,
        )
        .unwrap();

        assert_eq!(info1.id, info2.id);

        let m = mgr.lock().unwrap();
        // main + reuse_test = 2 shells
        assert_eq!(m.shells.len(), 2);
    }
}
