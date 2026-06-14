use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::app::PluginMeta;
use crate::constants::PLUGIN_TIMEOUT_SECS;
use crate::error::NashellError;
use crate::plugin::protocol::{send_message, PluginCall, PluginMessage, PluginOff, PluginResponse};

/// 插件进程句柄，维护与单个插件子进程的通信。
#[derive(Debug)]
pub struct PluginHandle {
    /// 插件元数据
    pub meta: PluginMeta,
    /// 子进程句柄
    pub child: Child,
}

impl PluginHandle {
    /// 获取插件名称。
    pub fn name(&self) -> &str {
        &self.meta.name
    }

    /// 检查子进程是否仍在运行。
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }
}

/// 插件管理器，负责所有插件进程的生命周期管理。
///
/// 管理插件启动、通信、关闭。维护一个 handle 表，按插件名索引。
pub struct PluginManager {
    /// 所有活跃的插件句柄，按插件名索引
    handles: HashMap<String, PluginHandle>,
}

impl PluginManager {
    /// 创建空的插件管理器。
    pub fn new() -> Self {
        PluginManager {
            handles: HashMap::new(),
        }
    }

    /// 启动一个插件进程。
    ///
    /// 启动插件可执行文件，其 stdin/stdout 用于 NDJSON 通信。
    /// exec 字段可以包含参数（如 "python3 plugin.py"），会按空格分割处理。
    ///
    /// # 参数
    /// - `meta`: 插件元数据
    ///
    /// # 返回
    /// 启动成功后的插件名称（可通过 get_handle 获取句柄）
    ///
    /// # 错误
    /// - 插件可执行文件不存在或无法启动
    pub fn start_plugin(&mut self, meta: PluginMeta) -> Result<String, NashellError> {
        let name = meta.name.clone();

        let parts: Vec<&str> = meta.exec.split_whitespace().collect();
        if parts.is_empty() {
            return Err(NashellError::Plugin {
                plugin_name: name.clone(),
                detail: "插件 exec 字段为空".to_string(),
            });
        }

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let child = cmd.spawn().map_err(|e| NashellError::Plugin {
            plugin_name: name.clone(),
            detail: format!("无法启动插件进程 '{}': {}", meta.exec, e),
        })?;

        let handle = PluginHandle {
            meta,
            child,
        };

        self.handles.insert(name.clone(), handle);
        Ok(name)
    }

    /// 获取指定插件的句柄。
    pub fn get_handle(&mut self, name: &str) -> Option<&mut PluginHandle> {
        self.handles.get_mut(name)
    }

    /// 向插件发送 call 消息。
    ///
    /// # 参数
    /// - `handle`: 插件进程句柄
    /// - `call`: call 消息数据
    ///
    /// # 错误
    /// - 写入失败（插件进程可能已退出）
    pub fn send_call(
        handle: &mut PluginHandle,
        call: &PluginCall,
    ) -> Result<(), NashellError> {
        let msg = PluginMessage::Call {
            sender: "nashell".to_string(),
            data: call.clone(),
        };

        let plugin_name = handle.name().to_string();
        let stdin = handle
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: "无法获取插件 stdin".to_string(),
            })?;

        send_message(stdin, &msg)
    }

    /// 发送包含 exec_result 的 response 消息给插件（toExec 完成后回传结果）。
    ///
    /// # 参数
    /// - `handle`: 插件进程句柄
    /// - `resp`: 包含 exec_result 的 response 消息
    pub fn send_response(
        handle: &mut PluginHandle,
        resp: &PluginResponse,
    ) -> Result<(), NashellError> {
        let msg = PluginMessage::Response {
            sender: "nashell".to_string(),
            data: resp.clone(),
        };

        let plugin_name = handle.name().to_string();
        let stdin = handle
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: "无法获取插件 stdin".to_string(),
            })?;

        send_message(stdin, &msg)
    }

    /// 从插件接收 response 消息直到收到 off 消息。
    ///
    /// 此函数阻塞等待插件回复，处理 toExec 请求并回传 exec_result。
    /// 当 response 的 is_print 为 true 且 streaming 为 true 时，
    /// out_content 会实时写入 out_writer（实现流式输出）。
    /// 收到 off 消息后返回收集到的所有 response 和 off 数据。
    ///
    /// # 参数
    /// - `handle`: 插件进程句柄
    /// - `out_writer`: 用于实时流式输出的 writer
    /// - `shell_type`: 当前 shell 类型
    /// - `timeout_secs`: shell 命令超时
    /// - `deny_patterns`: 安全拦截模式
    /// - `registry`: 命令注册表
    /// - `shell_manager`: Shell 管理器
    ///
    /// # 返回
    /// - `(Vec<PluginResponse>, PluginOff)`: 所有 response 消息和最终的 off 消息
    ///
    /// # 错误
    /// - 接收超时
    /// - 插件进程意外退出
    pub fn recv_responses(
        handle: &mut PluginHandle,
        out_writer: &mut dyn std::io::Write,
        shell_type: &str,
        timeout_secs: u64,
        deny_patterns: &[String],
        registry: &crate::nacommand::registry::CommandRegistry,
        shell_manager: Option<std::sync::Arc<std::sync::Mutex<crate::shell::manager::ShellManager>>>,
    ) -> Result<(Vec<PluginResponse>, PluginOff), NashellError> {
        let mut responses = Vec::new();
        let plugin_name = handle.name().to_string();

        // Take stdout out of handle so we can hold a BufReader for the entire session.
        let stdout = handle
            .child
            .stdout
            .take()
            .ok_or_else(|| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: "无法获取插件 stdout".to_string(),
            })?;

        let mut reader = BufReader::new(stdout);

        loop {
            let mut line = String::new();
            loop {
                line.clear();
                let bytes = reader.read_line(&mut line).map_err(|e| NashellError::Io {
                    path: None,
                    source: e,
                })?;
                if bytes == 0 {
                    return Err(NashellError::Plugin {
                        plugin_name: plugin_name.clone(),
                        detail: "插件连接意外关闭（读取到 EOF）".to_string(),
                    });
                }
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    break;
                }
            }

            let msg: PluginMessage =
                serde_json::from_str(line.trim()).map_err(|e| NashellError::Plugin {
                    plugin_name: plugin_name.clone(),
                    detail: format!("无法解析插件消息: {} (原始: {})", e, line.trim()),
                })?;

            match msg {
                PluginMessage::Response { data, .. } => {
                    // Handle to_exec requests
                    if !data.to_exec.is_empty() {
                        let exec_result = super::toexec::execute_toplevel(
                            &data.to_exec,
                            1,
                            shell_type,
                            timeout_secs,
                            deny_patterns,
                            registry,
                            shell_manager.clone(),
                        )?;

                        let resp_with_result = PluginResponse {
                            streaming: false,
                            out_content: String::new(),
                            out_prompt: None,
                            is_print: false,
                            to_exec: vec![],
                            exec_result: Some(exec_result),
                        };
                        Self::send_response(handle, &resp_with_result)?;
                    }

                    // Real-time streaming: print immediately if is_print
                    if data.is_print {
                        if let Some(ref prompt) = data.out_prompt {
                            let _ = writeln!(out_writer, "{}", prompt);
                        }
                        let _ = write!(out_writer, "{}", data.out_content);
                        // Ensure streaming content is flushed and visible immediately
                        let _ = writeln!(out_writer);
                        let _ = out_writer.flush();
                    }

                    responses.push(data);
                }
                PluginMessage::Off { data, .. } => {
                    handle.child.stdout = Some(reader.into_inner());
                    return Ok((responses, data));
                }
                _ => {
                    handle.child.stdout = Some(reader.into_inner());
                    return Err(NashellError::Plugin {
                        plugin_name: plugin_name.clone(),
                        detail: "收到意外的消息类型（期望 response 或 off）".to_string(),
                    });
                }
            }
        }
    }
    /// 关闭指定插件进程。
    ///
    /// 先尝试优雅关闭（关闭 stdin 等待进程自行退出），超时或失败则强制 kill。
    ///
    /// # 参数
    /// - `handle`: 插件进程句柄
    pub fn stop_plugin(handle: &mut PluginHandle) {
        let plugin_name = handle.name().to_string();

        // 关闭 stdin，让插件检测到 EOF 后自行退出
        drop(handle.child.stdin.take());

        // 等待插件进程退出（最多 PLUGIN_TIMEOUT_SECS 秒）
        let wait_result = wait_timeout(&mut handle.child, PLUGIN_TIMEOUT_SECS);

        match wait_result {
            Ok(true) => {
                log::info!("插件 '{}' 正常退出", plugin_name);
            }
            Ok(false) => {
                log::warn!("插件 '{}' 超时未退出，强制终止", plugin_name);
                let _ = handle.child.kill();
                let _ = handle.child.wait();
            }
            Err(e) => {
                log::warn!("等待插件 '{}' 退出时出错: {}，强制终止", plugin_name, e);
                let _ = handle.child.kill();
                let _ = handle.child.wait();
            }
        }
    }

    /// 关闭所有活跃的插件进程。
    pub fn stop_all(&mut self) {
        let names: Vec<String> = self.handles.keys().cloned().collect();
        for name in names {
            if let Some(handle) = self.handles.get_mut(&name) {
                Self::stop_plugin(handle);
            }
        }
        self.handles.clear();
    }

    /// 获取所有活跃的插件句柄列表（可变引用）。
    pub fn handles_mut(&mut self) -> impl Iterator<Item = &mut PluginHandle> {
        self.handles.values_mut()
    }
}

/// 等待子进程退出，最多等待 `timeout_secs` 秒。
///
/// 返回 `Ok(true)` 表示进程已退出，`Ok(false)` 表示超时。
fn wait_timeout(child: &mut Child, timeout_secs: u64) -> Result<bool, std::io::Error> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait()? {
            Some(_) => return Ok(true),
            None => {
                if start.elapsed() >= timeout {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_new_is_empty() {
        let mut mgr = PluginManager::new();
        assert!(mgr.handles.is_empty());
        assert!(mgr.handles_mut().next().is_none());
    }

    #[test]
    fn test_start_plugin_fails_for_nonexistent() {
        let mut mgr = PluginManager::new();
        let meta = PluginMeta {
            name: "nonexistent".to_string(),
            exec: "/nonexistent/plugin/binary".to_string(),
            is_broadcast: false,
            commands: vec![],
        };
        let result = mgr.start_plugin(meta);
        assert!(result.is_err());
    }

    #[test]
    fn test_stop_all_on_empty_manager() {
        let mut mgr = PluginManager::new();
        mgr.stop_all();
        assert!(mgr.handles.is_empty());
    }

    #[test]
    fn test_get_handle_nonexistent() {
        let mut mgr = PluginManager::new();
        assert!(mgr.get_handle("nonexistent").is_none());
    }
}
