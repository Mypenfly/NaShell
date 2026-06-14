use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app::PluginMeta;
use crate::constants::{PLUGIN_RECV_TIMEOUT_SECS, PLUGIN_TIMEOUT_SECS};
use crate::error::NashellError;
use crate::plugin::protocol::{
    recv_message, send_message, GetInput, PluginCall, PluginMessage, PluginOff, PluginResponse,
};
use crate::repl::prompt::colorize;

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
    /// 启动一个看门狗线程防止插件响应超时。超时后强制终止插件进程。
    /// response 消息中的 streaming 输出实时打印到 stdout。
    /// 收到 off 消息后返回收集到的所有 response 和 off 数据。
    ///
    /// # 参数
    /// - `handle`: 插件进程句柄
    /// - `out_writer`: 用于 off 消息最终输出的 writer（由调用方在 recv 后使用）
    /// - `shell_type`: 当前 shell 类型
    /// - `timeout_secs`: shell 命令超时
    /// - `deny_patterns`: 安全拦截模式
    /// - `registry`: 命令注册表
    /// - `shell_manager`: Shell 管理器
    ///
    /// # 返回
    /// - `(Vec<PluginResponse>, PluginOff)`: 所有 response 和最终的 off 消息
    ///
    /// # 错误
    /// - 插件响应超时
    /// - 插件进程意外退出
    /// - 收到非预期的消息类型
    pub fn recv_responses(
        handle: &mut PluginHandle,
        out_writer: &mut dyn std::io::Write,
        shell_type: &str,
        timeout_secs: u64,
        deny_patterns: &[String],
        registry: &crate::nacommand::registry::CommandRegistry,
        shell_manager: Option<std::sync::Arc<std::sync::Mutex<crate::shell::manager::ShellManager>>>,
    ) -> Result<(Vec<PluginResponse>, PluginOff), NashellError> {
        let plugin_name = handle.name().to_string();
        let stdout = handle
            .child
            .stdout
            .take()
            .ok_or_else(|| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: "无法获取插件 stdout".to_string(),
            })?;

        // 看门狗线程：超时后强制终止插件进程
        let timed_out = Arc::new(AtomicBool::new(false));
        let pid = handle.child.id();
        {
            let timed_out = timed_out.clone();
            let watchdog_name = plugin_name.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(PLUGIN_RECV_TIMEOUT_SECS));
                timed_out.store(true, Ordering::SeqCst);
                log::warn!(
                    "插件 '{}' 响应超时 ({}s)，强制终止 pid={}",
                    watchdog_name,
                    PLUGIN_RECV_TIMEOUT_SECS,
                    pid
                );
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                    std::thread::sleep(Duration::from_secs(2));
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            });
        }

        let reader = BufReader::new(stdout);
        let result = Self::read_plugin_responses(
            reader, handle, shell_type, timeout_secs,
            deny_patterns, registry, shell_manager,
        );

        // 流式输出已在 read_plugin_responses 打印至 stdout，
        // out_writer 由调用方在 recv 后用于 off 消息输出
        let _ = out_writer;

        match result {
            Ok(resp_and_off) => Ok(resp_and_off),
            Err(e) => {
                if timed_out.load(Ordering::SeqCst) {
                    let _ = handle.child.kill();
                    let _ = handle.child.wait();
                    Err(NashellError::Timeout {
                        command: plugin_name,
                        seconds: PLUGIN_RECV_TIMEOUT_SECS,
                    })
                } else {
                    Err(e)
                }
            }
        }
    }

    /// 读取插件响应消息直至收到 off。
    ///
    /// 由 `recv_responses` 调用，执行实际的阻塞读取循环。
    /// 使用 `protocol::recv_message` 按 NDJSON 帧读取消息。
    /// 处理 toExec 请求、回传 exec_result、实时流式打印。
    fn read_plugin_responses(
        mut reader: BufReader<std::process::ChildStdout>,
        handle: &mut PluginHandle,
        shell_type: &str,
        timeout_secs: u64,
        deny_patterns: &[String],
        registry: &crate::nacommand::registry::CommandRegistry,
        shell_manager: Option<Arc<std::sync::Mutex<crate::shell::manager::ShellManager>>>,
    ) -> Result<(Vec<PluginResponse>, PluginOff), NashellError> {
        let mut responses = Vec::new();
        let plugin_name = handle.name().to_string();

        loop {
            let msg = recv_message(&mut reader).map_err(|e| NashellError::Plugin {
                plugin_name: plugin_name.clone(),
                detail: format!("接收插件消息失败: {}", e),
            })?;

            match msg {
                PluginMessage::Response { data, .. } => {
                    // Handle to_exec
                    if !data.to_exec.is_empty() {
                        let exec_result = super::toexec::execute_toplevel(
                            &data.to_exec, 1, shell_type, timeout_secs,
                            deny_patterns, registry, shell_manager.clone(),
                        )?;
                        let resp_with_result = PluginResponse {
                            streaming: false,
                            out_content: String::new(),
                            out_prompt: None,
                            prompt_fg: "gray".to_string(),
                            is_print: false,
                            to_exec: vec![],
                            exec_result: Some(exec_result),
                            get_input: None,
                            user_input: None,
                        };
                        Self::send_response(handle, &resp_with_result)?;
                    }
                    // Handle get_input: pause, display prompt, collect user input
                    if let Some(ref gi) = data.get_input {
                        let user_input = request_user_input(gi)?;
                        let resp_with_input = PluginResponse {
                            streaming: true,
                            out_content: String::new(),
                            out_prompt: None,
                            prompt_fg: "gray".to_string(),
                            is_print: false,
                            to_exec: vec![],
                            exec_result: None,
                            get_input: None,
                            user_input: Some(user_input),
                        };
                        Self::send_response(handle, &resp_with_input)?;
                    }
                    print_plugin_response(&data);
                    responses.push(data);
                }
                PluginMessage::Off { data, .. } => {
                    handle.child.stdout = Some(reader.into_inner());
                    return Ok((responses, data));
                }
                _ => {
                    handle.child.stdout = Some(reader.into_inner());
                    return Err(NashellError::Plugin {
                        plugin_name,
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

/// 将插件的单条 response 消息实时打印到 stdout。
///
/// 若 `is_print` 为 true，则以 `prompt_fg` 颜色打印 out_prompt（如有）和 out_content。
fn print_plugin_response(data: &PluginResponse) {
    if !data.is_print {
        return;
    }
    let mut stdout = std::io::stdout();
    if let Some(ref prompt) = data.out_prompt {
        let colored = crate::repl::prompt::colorize(prompt, &data.prompt_fg);
        let _ = writeln!(stdout, "{}", colored);
    }
    let _ = write!(stdout, "{}", data.out_content);
    let _ = writeln!(stdout);
    let _ = stdout.flush();
}

/// 等待子进程退出，最多等待 `timeout_secs` 秒。

/// 向用户请求交互输入。
///
/// 显示 pre_content 和 input_prompt，收集用户的多行输入。
/// 首行为 input_prompt，续行提示符为 `>> `（Enter 空行提交）。
fn request_user_input(gi: &GetInput) -> Result<String, NashellError> {
    let mut stdout = std::io::stdout();
    let stdin = std::io::stdin();

    if let Some(ref pre) = gi.pre_content {
        let colored = colorize(pre, &gi.pre_fg);
        let _ = writeln!(stdout, "{}", colored);
    }

    let colored_prompt = colorize(&gi.input_prompt, &gi.input_fg);
    let _ = write!(stdout, "{}", colored_prompt);
    let _ = stdout.flush();

    let mut lines = Vec::new();
    let mut buf = String::new();
    stdin.read_line(&mut buf).map_err(|e| NashellError::Io {
        path: None,
        source: e,
    })?;

    let first = buf.trim_end_matches(|c| c == '\n' || c == '\r').to_string();
    lines.push(first.clone());
    buf.clear();

    // Continue reading until empty line (multi-line input)
    let cont_prompt = colorize(">> ", "");
    loop {
        let _ = write!(stdout, "{}", cont_prompt);
        let _ = stdout.flush();
        buf.clear();
        stdin.read_line(&mut buf).map_err(|e| NashellError::Io {
            path: None,
            source: e,
        })?;
        let trimmed = buf.trim_end_matches(|c| c == '\n' || c == '\r').to_string();
        if trimmed.is_empty() {
            break;
        }
        lines.push(trimmed);
    }

    Ok(lines.join("\n"))
}

/// 等待子进程退出，最多等待 `timeout_secs` 秒。，最多等待 `timeout_secs` 秒。
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
