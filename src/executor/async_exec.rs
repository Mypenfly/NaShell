use std::sync::{Arc, Mutex};
use std::path::PathBuf;

use crate::error::NashellError;
use crate::nacommand::registry::CommandRegistry;
use crate::parser::syntax::RawCommands;
use crate::plugin::manager::PluginManager;
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
/// 在后台线程中走完整的 NaShell 解析→分派流程，与用户输入的处理完全一致。
/// 支持 NaCommand（含 Write、Open、Bash 等）、管道、long_argument 等所有特性。
/// 结果存入指定 shell 的 pools。
/// 若同名 shell 已存在则复用，否则创建新 shell。
///
/// # 参数
/// - `name`: 异步 shell 的名称（来自 @/Async(name)）
/// - `raw_commands`: 已解析的命令集合
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
/// - `timeout_secs`: 命令超时秒数
/// - `deny_patterns`: 安全拦截模式列表
/// - `manager`: Shell 管理器（共享引用）
/// - `registry`: 命令注册表
/// - `plugin_manager`: 插件管理器（可选共享引用）
/// - `cwd`: 当前工作目录快照
pub fn spawn_async_shell_exec(
    name: &str,
    raw_commands: &RawCommands,
    shell_type: &str,
    timeout_secs: u64,
    deny_patterns: &[String],
    manager: &Arc<Mutex<ShellManager>>,
    registry: CommandRegistry,
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
    cwd: &PathBuf,
) -> Result<AsyncExecInfo, NashellError> {
    let (shell_id, shell_name) = {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: name.to_string(),
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
    let raw_commands = raw_commands.clone();
    let shell_type_owned = shell_type.to_string();
    let manager_clone = Arc::clone(manager);
    let shell_id_clone = shell_id.clone();
    let deny_patterns_owned = deny_patterns.to_vec();

    let handle = std::thread::spawn(move || {
        execute_pipeline(
            &raw_commands,
            &shell_type_owned,
            timeout_secs,
            &deny_patterns_owned,
            &manager_clone,
            &shell_id_clone,
            &registry,
            &plugin_manager,
        );
    });

    // 保存线程句柄
    {
        let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
            command: name.to_string(),
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

/// 在后台线程中执行完整的命令管道。
///
/// 逐段走 `dispatch()` 分派，前段输出通过 pre_out 传递到后段。
/// 管道最终输出存入 pools。
fn execute_pipeline(
    raw_commands: &RawCommands,
    shell_type: &str,
    timeout_secs: u64,
    deny_patterns: &[String],
    manager_clone: &Arc<Mutex<ShellManager>>,
    shell_id: &str,
    registry: &CommandRegistry,
    plugin_manager: &Option<Arc<Mutex<PluginManager>>>,
) {
    let mut pre_out: Option<String> = None;

    for (i, cmd) in raw_commands.commands.iter().enumerate() {
        let mut ctx = crate::executor::ExecContext {
            shell_type: shell_type.to_string(),
            pre_out: pre_out.clone(),
            timeout_secs,
            deny_patterns: deny_patterns.to_vec(),
            long_argument: if i == 0 {
                raw_commands.long_argument.clone()
            } else {
                None
            },
            registry: Some(registry.clone()),
            shell_manager: Some(manager_clone.clone()),
            plugin_manager: plugin_manager.clone(),
        };

        let mut out_buf = Vec::new();
        match crate::executor::dispatch(cmd, &mut ctx, &mut out_buf) {
            Ok((output, _)) => {
                pre_out = Some(output);
            }
            Err(e) => {
                let err_output = crate::error::display::format_error(&e);
                if let Ok(mut mgr) = manager_clone.lock() {
                    let _ = mgr.add_to_pools(shell_id, &err_output);
                }
                return;
            }
        }
    }

    if let Some(output) = pre_out {
        let output = output.trim().to_string();
        if !output.is_empty() {
            if let Ok(mut mgr) = manager_clone.lock() {
                let _ = mgr.add_to_pools(shell_id, &output);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::syntax::{CmdType, RawCmd, RawCommands};
    use crate::app::{CmdMeta, Level};

    fn test_manager() -> Arc<Mutex<ShellManager>> {
        let mut mgr = ShellManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.register_main(&cwd);
        Arc::new(Mutex::new(mgr))
    }

    fn test_registry() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.register_builtin(CmdMeta {
            level: Level::Normal,
            name: "write".to_string(),
            exec: "n_write".to_string(),
            long_argument: true,
            exec_script: None,
            known_modes: vec!["help".to_string()],
        });
        registry
    }

    #[test]
    fn test_spawn_async_shell_exec() {
        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();
        let registry = test_registry();

        let raw = RawCommands {
            commands: vec![RawCmd {
                cmd_type: CmdType::Shell,
                cmd: "echo".to_string(),
                args: vec!["hello_async".to_string()],
            }],
            long_argument: None,
            pre_out: None,
            async_name: None,
        };

        let info = spawn_async_shell_exec(
            "test_async",
            &raw,
            "bash",
            120,
            &[],
            &mgr,
            registry,
            None,
            &cwd,
        )
        .unwrap();

        assert_eq!(info.name, "test_async");
        assert!(!info.id.is_empty());

        let m = mgr.lock().unwrap();
        assert!(m.shells.contains_key(&info.id));
    }

    #[test]
    fn test_spawn_async_shell_reuse() {
        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();
        let registry = test_registry();

        let raw = RawCommands {
            commands: vec![RawCmd {
                cmd_type: CmdType::Shell,
                cmd: "echo".to_string(),
                args: vec!["first".to_string()],
            }],
            long_argument: None,
            pre_out: None,
            async_name: Some("reuse_test".to_string()),
        };

        let info1 = spawn_async_shell_exec(
            "reuse_test",
            &raw,
            "bash",
            120,
            &[],
            &mgr,
            registry.clone(),
            None,
            &cwd,
        )
        .unwrap();

        let info2 = spawn_async_shell_exec(
            "reuse_test",
            &raw,
            "bash",
            120,
            &[],
            &mgr,
            registry,
            None,
            &cwd,
        )
        .unwrap();

        assert_eq!(info1.id, info2.id);

        let m = mgr.lock().unwrap();
        assert_eq!(m.shells.len(), 2);
    }

    #[test]
    fn test_spawn_async_na_command() {
        let dir = std::env::temp_dir().join(format!("nashell_async_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("async_test.txt");

        let mgr = test_manager();
        let cwd = std::env::current_dir().unwrap();
        let registry = test_registry();

        let raw = RawCommands {
            commands: vec![RawCmd {
                cmd_type: CmdType::NaCommandNormal,
                cmd: "Write".to_string(),
                args: vec![file_path.to_string_lossy().to_string()],
            }],
            long_argument: Some("async written content".to_string()),
            pre_out: None,
            async_name: None,
        };

        let _info = spawn_async_shell_exec(
            "write_test",
            &raw,
            "bash",
            120,
            &[],
            &mgr,
            registry,
            None,
            &cwd,
        )
        .unwrap();

        // Give the thread time to execute
        std::thread::sleep(std::time::Duration::from_secs(1));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "async written content");
    }
}
