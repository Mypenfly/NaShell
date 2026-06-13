// PTY 模块供后续 Phase（异步 Shell、交互命令）使用，
// 当前主执行路径使用 script -q -c 方案，不依赖本模块。
#![allow(dead_code)]

use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};

use crate::constants;
use crate::error::NashellError;

/// PROMPT_COMMAND 发出的命令完成标记（OSC 777 自定义序列）
const DONE_MARKER: &[u8] = b"\x1b]777;done\x07";

/// 初始化就绪探测字符串（纯文本，避免与 OSC 混淆）
const READY_MARKER: &[u8] = b"__NASHELL_READY__";

/// PTY 初始化超时
const INIT_TIMEOUT_SECS: u64 = 15;

/// 命令执行超时
const CMD_TIMEOUT_SECS: u64 = 30;

/// 轮询间隔（毫秒）
const POLL_INTERVAL_MS: u64 = 10;

/// PTY 会话，持有 PTY 的读写端和子进程信息。
pub struct PtySession {
    /// 子进程 PID
    pub child_pid: u32,
    /// PTY 读取端（显式设置了 O_NONBLOCK）
    reader: File,
    /// PTY 写入端
    writer: Box<dyn Write + Send>,
}

impl fmt::Debug for PtySession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PtySession")
            .field("child_pid", &self.child_pid)
            .finish()
    }
}

/// 对 fd 设置 O_NONBLOCK 标志。
///
/// # Safety 合理性
/// 必须通过 `libc::fcntl` 设置 `O_NONBLOCK`，因为 `portable-pty` 打开的
/// PTY master fd 默认会继承阻塞模式。Rust 标准库的 `File` 没有直接设置
/// 非阻塞模式的方法。`libc::fcntl` 是 POSIX 标准调用，在此上下文中安全。
fn set_nonblocking(fd: std::os::unix::io::RawFd) -> Result<(), NashellError> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags == -1 {
        return Err(NashellError::Io {
            path: Some("fcntl F_GETFL".to_string()),
            source: std::io::Error::last_os_error(),
        });
    }
    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if ret == -1 {
        return Err(NashellError::Io {
            path: Some("fcntl F_SETFL O_NONBLOCK".to_string()),
            source: std::io::Error::last_os_error(),
        });
    }
    Ok(())
}

/// 在 PTY 中启动 shell 并完成初始化。
///
/// 初始化流程：
/// 1. 创建 PTY，启动 shell
/// 2. 注入初始化命令（stty、PS1、PROMPT_COMMAND）
/// 3. 等待就绪确认信号
///
/// # Safety 合理性
/// 使用 `libc::dup` 复制 PTY master fd，因为需要独立的 reader/writer。
/// `portable-pty` 的 master 端共享同一个 fd，直接对读/写操作会有竞争。
/// `dup` 是 POSIX 标准操作，创建的 fd 生命周期由 `File::from_raw_fd`
/// 管理，不会泄漏。`from_raw_fd` 在此处安全因为 dup 出来的 fd 是有效的。
pub fn spawn_pty_session(shell_type: &str) -> Result<PtySession, NashellError> {
    let pty_system = native_pty_system();

    let pty_size = PtySize {
        rows: constants::DEFAULT_PTY_ROWS,
        cols: constants::DEFAULT_PTY_COLS,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pair = pty_system.openpty(pty_size).map_err(|e| NashellError::Io {
        path: Some("openpty".to_string()),
        source: std::io::Error::other(format!("{:#}", e)),
    })?;

    let cmd = CommandBuilder::new(shell_type);
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| NashellError::Io {
            path: Some("spawn_command".to_string()),
            source: std::io::Error::other(format!("{:#}", e)),
        })?;

    let child_pid = child
        .process_id()
        .ok_or_else(|| NashellError::Execute {
            command: shell_type.to_string(),
            exit_code: None,
            stderr: "无法获取子进程 PID".to_string(),
        })?;

    let (mut reader, mut writer) = create_pty_io_pair(&pair, shell_type)?;

    init_shell_session(&mut writer)?;

    wait_for_shell_ready(&mut reader)?;

    Ok(PtySession {
        child_pid,
        reader,
        writer,
    })
}

/// 从 PTY pair 创建独立的 reader/writer 端点。
///
/// 复制 master fd 以避免读写竞争，reader 设置为非阻塞模式。
///
/// # Safety 合理性
/// `libc::dup` 复制 fd 以确保 reader 和 writer 使用独立的文件描述符。
/// 共享同一个 fd 会导致读写操作相互干扰。`dup` 是 POSIX 标准操作，
/// 新 fd 的生命周期由返回的 `File` 管理，不会泄漏资源。
fn create_pty_io_pair(
    pair: &PtyPair,
    shell_type: &str,
) -> Result<(File, Box<dyn Write + Send>), NashellError> {
    let master_fd = pair.master.as_raw_fd();
    let fd = master_fd.ok_or_else(|| NashellError::Execute {
        command: shell_type.to_string(),
        exit_code: None,
        stderr: "无法获取 PTY master fd".to_string(),
    })?;
    let dup_fd = unsafe { libc::dup(fd) };
    if dup_fd == -1 {
        return Err(NashellError::Io {
            path: Some("dup".to_string()),
            source: std::io::Error::last_os_error(),
        });
    }
    set_nonblocking(dup_fd)?;
    let reader = unsafe { File::from_raw_fd(dup_fd) };

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| NashellError::Io {
            path: Some("take_writer".to_string()),
            source: std::io::Error::other(format!("{:#}", e)),
        })?;

    Ok((reader, writer))
}

/// 向新启动的 shell 注入初始化命令。
///
/// 注入内容：
/// - `stty -echo` — 抑制命令回显
/// - `export PS1=''` — 清空提示符避免文本干扰
/// - `export PROMPT_COMMAND` — 注入 OSC 777 完成标记
/// - `echo __NASHELL_READY__` — 就绪探测
fn init_shell_session(writer: &mut (impl Write + ?Sized)) -> Result<(), NashellError> {
    let init_cmds = format!(
        "stty -echo\n\
         export PS1=''\n\
         export PROMPT_COMMAND='printf \"\\x1b]777;done\\x07\"'\n\
         echo __NASHELL_READY__\n"
    );
    writer
        .write_all(init_cmds.as_bytes())
        .map_err(|e| NashellError::Io {
            path: None,
            source: e,
        })?;
    writer.flush().map_err(|e| NashellError::Io {
        path: None,
        source: e,
    })
}

/// 等待 shell 初始化完成信号。
///
/// 轮询 PTY 输出直到同时检测到就绪标记和 PROMPT_COMMAND 完成标记，
/// 确保所有初始化输出在返回前被消费。
fn wait_for_shell_ready(reader: &mut File) -> Result<(), NashellError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(INIT_TIMEOUT_SECS);
    let mut found_ready = false;
    let mut found_done = false;
    let mut init_buf: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];

    while std::time::Instant::now() < deadline && !found_done {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                init_buf.extend_from_slice(&buf[..n]);
                if !found_ready && has_subslice(&init_buf, READY_MARKER) {
                    found_ready = true;
                    log::debug!("PTY init: ready marker found");
                }
                if found_ready && has_subslice(&init_buf, DONE_MARKER) {
                    found_done = true;
                    log::debug!("PTY init: done marker found after ready");
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => {
                return Err(NashellError::Io {
                    path: None,
                    source: e,
                });
            }
        }
    }

    // 排空残留数据
    drain_reader(reader);

    if !found_ready {
        log::warn!(
            "PTY 初始化未在 {}s 内收到就绪信号，继续尝试",
            INIT_TIMEOUT_SECS
        );
    } else {
        log::debug!("PTY 初始化完成，shell 已就绪");
    }

    Ok(())
}

/// 向 PTY 发送命令，等待执行完毕并返回输出。
///
/// 工作流程：
/// 1. 排空 PTY 读端残留数据
/// 2. 写入命令文本
/// 3. 循环读取 PTY 输出，直到 PROMPT_COMMAND 发出的 OSC 完成标记出现
/// 4. 从输出末尾移除标记和命令回显，返回清理后的输出
pub fn send_command(session: &mut PtySession, command: &str) -> Result<String, NashellError> {
    // 1. 排空残留数据
    drain_reader(&mut session.reader);

    // 2. 写入命令
    session
        .writer
        .write_all(command.as_bytes())
        .map_err(|e| NashellError::Io {
            path: None,
            source: e,
        })?;
    session
        .writer
        .write_all(b"\n")
        .map_err(|e| NashellError::Io {
            path: None,
            source: e,
        })?;
    session.writer.flush().map_err(|e| NashellError::Io {
        path: None,
        source: e,
    })?;

    // 3. 读取直到完成标记出现
    let mut output: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let deadline = std::time::Instant::now() + Duration::from_secs(CMD_TIMEOUT_SECS);

    loop {
        match session.reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                output.extend_from_slice(&buf[..n]);
                if has_subslice(&output, DONE_MARKER) {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    log::warn!("PTY 命令超时: {}", command);
                    break;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => {
                return Err(NashellError::Io {
                    path: None,
                    source: e,
                });
            }
        }
    }

    // 4. 移除末尾的完成标记
    let cleaned = remove_last_marker(&output, DONE_MARKER);

    // 5. 尝试去除命令回显行（首行与输入命令相匹配时视为回显）
    let result = String::from_utf8_lossy(&cleaned);
    let result = if let Some(rest) = result.strip_prefix(&format!("{}\r\n", command)) {
        rest.to_string()
    } else if let Some(rest) = result.strip_prefix(&format!("{}\n", command)) {
        rest.to_string()
    } else {
        result.to_string()
    };

    let result = result
        .trim_end_matches(|c: char| c == '\r' || c == '\n')
        .to_string();

    Ok(result)
}

/// 非阻塞地排空 reader 中所有可读数据。
fn drain_reader(reader: &mut File) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
}

/// 检查 `haystack` 中是否包含子切片 `needle`（逐字节匹配）。
fn has_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// 从字节序列中移除最后一个出现的标记切片。
///
/// 返回截断后的字节序列（到标记之前）。若未找到标记，返回原始副本。
fn remove_last_marker(data: &[u8], marker: &[u8]) -> Vec<u8> {
    if let Some(pos) = data
        .windows(marker.len())
        .enumerate()
        .rev()
        .find(|(_, w)| *w == marker)
        .map(|(i, _)| i)
    {
        data[..pos].to_vec()
    } else {
        data.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_subslice_found() {
        let data = b"hello world\x1b]777;done\x07end";
        assert!(has_subslice(data, DONE_MARKER));
    }

    #[test]
    fn test_has_subslice_not_found() {
        let data = b"hello world";
        assert!(!has_subslice(data, DONE_MARKER));
    }

    #[test]
    fn test_remove_last_marker_single() {
        let data = b"output1\noutput2\n\x1b]777;done\x07";
        let cleaned = remove_last_marker(data, DONE_MARKER);
        assert_eq!(cleaned, b"output1\noutput2\n");
    }

    #[test]
    fn test_remove_last_marker_multiple() {
        let data = b"\x1b]777;done\x07output\x1b]777;done\x07";
        let cleaned = remove_last_marker(data, DONE_MARKER);
        assert_eq!(cleaned, b"\x1b]777;done\x07output");
    }

    #[test]
    fn test_remove_last_marker_not_found() {
        let data = b"plain output";
        let cleaned = remove_last_marker(data, DONE_MARKER);
        assert_eq!(cleaned, data);
    }

    #[test]
    fn test_spawn_pty_session_bash() {
        let result = spawn_pty_session("bash");
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_command_echo() {
        let mut session =
            spawn_pty_session("bash").expect("bash required");
        let output = send_command(&mut session, "echo HELLO_NASHELL")
            .expect("send_command failed");
        assert!(
            output.contains("HELLO_NASHELL"),
            "expected 'HELLO_NASHELL' in output, got: '{}'",
            output
        );
    }

    #[test]
    fn test_send_command_cd_and_pwd() {
        let mut session =
            spawn_pty_session("bash").expect("bash required");
        let _ = send_command(&mut session, "cd /tmp");
        let output = send_command(&mut session, "pwd")
            .expect("send_command failed");
        assert!(
            output.contains("/tmp"),
            "expected '/tmp' in output, got: '{}'",
            output
        );
    }

    #[test]
    fn test_send_command_multiline_output() {
        let mut session =
            spawn_pty_session("bash").expect("bash required");
        let output = send_command(
            &mut session,
            "printf 'line1\\nline2\\nline3'",
        )
        .expect("send_command failed");
        assert!(
            output.contains("line1"),
            "expected multiline output, got: '{}'",
            output
        );
    }

    #[test]
    fn test_send_command_empty_output() {
        let mut session =
            spawn_pty_session("bash").expect("bash required");
        let output = send_command(&mut session, "true")
            .expect("send_command failed");
        assert!(
            output.is_empty(),
            "expected empty output for 'true', got: '{}'",
            output
        );
    }
}
