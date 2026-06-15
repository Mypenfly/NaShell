use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::error::NashellError;
use crate::shell::actor::Shell;
use crate::shell::cmd::ShellCmd;
use crate::shell::out::ShellOut;
use crate::shell::pty::{spawn_pty_session, PtySession};

/// Shell 管理器，维护 main shell 和所有异步 shell。
///
/// main shell（name="main"）是逻辑上的主 shell，其 path 与 Rust 进程 cwd 同步。
/// async shell 是用户通过 `@/Async(name)` 创建的后台 shell，持有独立的
/// 工作目录快照和后台线程句柄。
pub struct ShellManager {
    /// 主 shell 的 PTY 会话（当前架构下不持久化，保留供后续使用）
    pub main_session: Option<PtySession>,
    /// 所有 shell 集合，按 id 索引
    pub shells: HashMap<String, Shell>,
    /// 名称到 id 的映射
    pub name_to_id: HashMap<String, String>,
}

impl ShellManager {
    /// 创建空的管理器。
    pub fn new() -> Self {
        ShellManager {
            main_session: None,
            shells: HashMap::new(),
            name_to_id: HashMap::new(),
        }
    }

    /// 初始化主 shell PTY 会话。
    pub fn init_main_shell(&mut self, shell_type: &str) -> Result<(), NashellError> {
        let session = spawn_pty_session(shell_type)?;
        self.main_session = Some(session);
        Ok(())
    }

    /// 注册主 shell（name="main"）。
    ///
    /// 在 REPL 启动时调用。主 shell 不持有后台线程，
    /// 其 path 与 Rust 进程 cwd 保持同步。
    ///
    /// # 参数
    /// - `cwd`: 当前工作目录
    pub fn register_main(&mut self, cwd: &PathBuf) {
        let id = generate_id();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<ShellCmd>();
        let (out_tx, _out_rx) = mpsc::channel::<ShellOut>();

        let shell = Shell {
            name: "main".to_string(),
            id: id.clone(),
            path: cwd.clone(),
            pty: None,
            cmd_rx,
            out_tx,
            pools: Vec::new(),
            join_handle: None,
        };

        self.name_to_id.insert("main".to_string(), id.clone());
        self.shells.insert(id, shell);
    }

    /// 同步主 shell 的工作目录。
    ///
    /// 每次 REPL 循环后调用，使 main shell 的 path 与实际 cwd 保持一致。
    pub fn sync_main_cwd(&mut self, cwd: &PathBuf) {
        if let Some(main_id) = self.name_to_id.get("main") {
            if let Some(shell) = self.shells.get_mut(main_id) {
                shell.path = cwd.clone();
            }
        }
    }

    /// 获取或创建指定名称的 shell。
    ///
    /// 若同名 shell 已存在则返回，否则创建新的 shell。
    /// 创建时使用当前工作目录快照作为 shell 的 path。
    ///
    /// # 参数
    /// - `name`: shell 名称（不含 "main"）
    /// - `cwd`: 创建时的当前工作目录
    ///
    /// # 返回
    /// 创建或找到的 shell 的 id
    pub fn get_or_create_shell(&mut self, name: &str, cwd: &PathBuf) -> String {
        if let Some(existing_id) = self.name_to_id.get(name) {
            return existing_id.clone();
        }

        let id = generate_id();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<ShellCmd>();
        let (out_tx, _out_rx) = mpsc::channel::<ShellOut>();

        let shell = Shell {
            name: name.to_string(),
            id: id.clone(),
            path: cwd.clone(),
            pty: None,
            cmd_rx,
            out_tx,
            pools: Vec::new(),
            join_handle: None,
        };

        self.name_to_id.insert(name.to_string(), id.clone());
        self.shells.insert(id.clone(), shell);
        id
    }

    /// 向指定 shell 的 pools 追加一条输出。
    ///
    /// # 参数
    /// - `shell_id`: shell 的唯一 id
    /// - `output`: 要追加的输出内容
    pub fn add_to_pools(&mut self, shell_id: &str, output: &str) -> Result<(), NashellError> {
        let shell = self
            .shells
            .get_mut(shell_id)
            .ok_or_else(|| NashellError::CommandNotFound {
                name: shell_id.to_string(),
                suggestion: None,
            })?;
        shell.pools.push(output.to_string());
        Ok(())
    }

    /// 设置指定 shell 的后台线程句柄。
    pub fn set_join_handle(&mut self, shell_id: &str, handle: std::thread::JoinHandle<()>) -> Result<(), NashellError> {
        let shell = self
            .shells
            .get_mut(shell_id)
            .ok_or_else(|| NashellError::CommandNotFound {
                name: shell_id.to_string(),
                suggestion: None,
            })?;
        shell.join_handle = Some(handle);
        Ok(())
    }

    /// 查看指定 shell 的 pools 中最近 `count` 条输出。
    ///
    /// # 参数
    /// - `shell_id`: shell 的唯一 id
    /// - `count`: 查看数量（从后往前取）
    ///
    /// # 返回
    /// pools 中最近 count 条输出（按从旧到新顺序）
    pub fn watch_pools(&self, shell_id: &str, count: usize) -> Result<Vec<String>, NashellError> {
        let shell = self
            .shells
            .get(shell_id)
            .ok_or_else(|| NashellError::CommandNotFound {
                name: shell_id.to_string(),
                suggestion: None,
            })?;

        let total = shell.pools.len();
        let actual_count = count.min(total);
        let start = total - actual_count;

        let result: Vec<String> = shell.pools[start..]
            .iter()
            .cloned()
            .collect();

        Ok(result)
    }

    /// 销毁指定 shell。
    ///
    /// 如果 shell 持有后台线程，等待其结束后移除 shell 条目。
    /// main shell 不能被销毁。
    ///
    /// # 参数
    /// - `shell_id`: shell 的唯一 id
    pub fn destroy_shell(&mut self, shell_id: &str) -> Result<(), NashellError> {
        let shell = self
            .shells
            .get_mut(shell_id)
            .ok_or_else(|| NashellError::CommandNotFound {
                name: shell_id.to_string(),
                suggestion: None,
            })?;

        if shell.name == "main" {
            return Err(NashellError::Execute {
                command: "destroy".to_string(),
                exit_code: None,
                stderr: "不能销毁 main shell".to_string(),
            });
        }

        // 等待后台线程结束
        if let Some(handle) = shell.join_handle.take() {
            let _ = handle.join();
        }

        self.name_to_id.remove(&shell.name);
        self.shells.remove(shell_id);

        Ok(())
    }

    /// 将 main shell 切换为指定 id 的 shell。
    ///
    /// 交换 main shell 和指定 shell 的工作目录（path）。
    /// 若 `destroy_old` 为 true，销毁旧的 main shell。
    ///
    /// # 参数
    /// - `shell_id`: 要切换到的目标 shell 的 id
    /// - `destroy_old`: 是否销毁旧的 main shell
    pub fn switch_main(&mut self, shell_id: &str, destroy_old: bool) -> Result<(), NashellError> {
        // 找到目标 shell 和 main shell
        let main_id = self
            .name_to_id
            .get("main")
            .cloned()
            .ok_or_else(|| NashellError::Execute {
                command: "switch".to_string(),
                exit_code: None,
                stderr: "main shell 不存在".to_string(),
            })?;

        if shell_id == main_id {
            return Err(NashellError::Execute {
                command: "switch".to_string(),
                exit_code: None,
                stderr: "不能切换到当前 main shell 自身".to_string(),
            });
        }

        // 获取目标 shell 信息（先克隆 path，避免借用冲突）
        let old_main_path;
        let target_name;
        let target_path;

        {
            let main_shell = self.shells.get(&main_id).ok_or_else(|| {
                NashellError::CommandNotFound {
                    name: main_id.clone(),
                    suggestion: None,
                }
            })?;
            let target_shell = self.shells.get(shell_id).ok_or_else(|| {
                NashellError::CommandNotFound {
                    name: shell_id.to_string(),
                    suggestion: None,
                }
            })?;

            old_main_path = main_shell.path.clone();
            target_name = target_shell.name.clone();
            target_path = target_shell.path.clone();
        }

        // 交换：目标 shell 获得 "main" 名称和旧 main 的 path
        {
            let target_shell = self.shells.get_mut(shell_id).ok_or_else(|| {
                NashellError::CommandNotFound {
                    name: shell_id.to_string(),
                    suggestion: None,
                }
            })?;
            target_shell.name = "main".to_string();
            target_shell.path = old_main_path.clone();
            self.name_to_id.remove(&target_name);
            self.name_to_id.insert("main".to_string(), shell_id.to_string());
        }

        // 旧 main 获得目标 shell 的旧名称和 path
        {
            let old_main = self.shells.get_mut(&main_id).ok_or_else(|| {
                NashellError::CommandNotFound {
                    name: main_id.clone(),
                    suggestion: None,
                }
            })?;
            old_main.name = target_name.clone();
            old_main.path = target_path;
            self.name_to_id.insert(target_name.clone(), main_id.clone());
        }

        // 同步 Rust 进程的 cwd 到新 main 的 path
        let result = std::env::set_current_dir(&old_main_path).map_err(|e| NashellError::Io {
            path: Some(old_main_path.display().to_string()),
            source: e,
        });

        if destroy_old {
            // 销毁旧的 main（现在已改名为 target_name）
            if let Some(old_main) = self.shells.get_mut(&main_id) {
                if let Some(handle) = old_main.join_handle.take() {
                    let _ = handle.join();
                }
            }
            self.name_to_id.remove(&target_name);
            self.shells.remove(&main_id);
        }

        result
    }

    /// 列出所有 shell 的状态信息。
    ///
    /// 返回 Vec 用于构建表格输出，每个元素为 (name, id, path, pools_count)。
    pub fn list_shells(&self) -> Vec<(String, String, String, usize)> {
        let mut result: Vec<_> = self
            .shells
            .iter()
            .map(|(id, shell)| {
                (
                    shell.name.clone(),
                    id.clone(),
                    shell.path.to_string_lossy().to_string(),
                    shell.pools.len(),
                )
            })
            .collect();

        // 按 name 排序，main 排在最前
        result.sort_by(|a, b| {
            if a.0 == "main" {
                std::cmp::Ordering::Less
            } else if b.0 == "main" {
                std::cmp::Ordering::Greater
            } else {
                a.0.cmp(&b.0)
            }
        });

        result
    }

    /// 获取指定 shell 的 name。
    pub fn get_shell_name(&self, shell_id: &str) -> Option<&str> {
        self.shells.get(shell_id).map(|s| s.name.as_str())
    }
}

/// 生成 8 字符的随机 hex id。
pub(crate) fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:08x}", (nanos & 0xFFFF_FFFF) as u32)
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
