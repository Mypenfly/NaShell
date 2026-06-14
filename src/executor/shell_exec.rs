use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use std::io::Write;

use crate::error::NashellError;

/// 捕获命令执行的结果。
#[derive(Debug, Clone)]
pub struct CapturedOutput {
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 退出码
    pub exit_code: i32,
}

/// 对命令文本做单引号转义，用于嵌入 `shell -c '...'`。
///
/// 规则：将内部 `'` 替换为 `'\''`（结束引用 → 转义单引号 → 恢复引用）。
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 从 `std::process::Output` 构建 `CapturedOutput`。
fn build_captured_output(output: std::process::Output) -> Result<CapturedOutput, NashellError> {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(CapturedOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// 等待子进程输出，支持超时机制。
///
/// 若 `timeout_secs == 0`，直接阻塞等待。
/// 若超时，通过 `libc::kill` 终止子进程并返回 `Timeout` 错误。
///
/// # Safety
/// `libc::kill` 对已知有效的子进程 PID 发送 SIGTERM/SIGKILL 是安全的，
/// 这是唯一能在 `wait_with_output` 消费 `Child` 所有权后仍能终止进程的方式。
fn wait_child_with_timeout(
    child: std::process::Child,
    command_label: &str,
    timeout_secs: u64,
) -> Result<CapturedOutput, NashellError> {
    if timeout_secs == 0 {
        let output = child.wait_with_output().map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?;
        return build_captured_output(output);
    }

    let child_pid = child.id();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(output)) => build_captured_output(output),
        Ok(Err(e)) => Err(NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        }),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let pid = child_pid as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            thread::sleep(Duration::from_millis(500));
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            log::warn!("命令超时 ({}s): {}", timeout_secs, command_label);
            Err(NashellError::Timeout {
                command: command_label.to_string(),
                seconds: timeout_secs,
            })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(NashellError::Execute {
            command: command_label.to_string(),
            exit_code: None,
            stderr: "子进程意外终止".to_string(),
        }),
    }
}

/// 执行 Bash 命令（`!!@Bash:`）。
///
/// 直接通过 `script -e -q -c "bash -c '{args}'" /dev/null` 执行，
/// 不做额外的 shell 类型包装。这是避免与 `exec_captured` 的
/// `{shell} -c '{cmd}'` 包装产生双层嵌套的关键。
///
/// # 参数
/// - `bash_args`: 传给 `bash -c` 的参数字符串
/// - `timeout_secs`: 超时秒数
pub fn exec_bash(
    bash_args: &str,
    timeout_secs: u64,
) -> Result<CapturedOutput, NashellError> {
    let inner_cmd = format!("bash -c {}", shell_quote(bash_args));

    let child = Command::new("script")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-e")
        .arg("-q")
        .arg("-c")
        .arg(&inner_cmd)
        .arg("/dev/null")
        .env("TERM", "xterm-256color")
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?;

    wait_child_with_timeout(child, bash_args, timeout_secs)
}

/// 通过 `script -q -c` 在 PTY 中执行 shell 命令并捕获输出。
///
/// `script` 分配伪终端。传入真实 stdin 使 `script` 能查询终端尺寸
///（解决默认 0×0 导致 nushell 表格渲染失败的问题）。
/// 命令结束后 `script` 自动退出，无持久会话、无哨兵、无状态机。
///
/// 如果命令超过 `timeout_secs` 秒未完成，强制终止并返回超时错误。
///
/// 执行链路：
///   Rust Command → script -e -q -c "{shell} -c '{command}'" /dev/null
///
/// # 参数
/// - `cmd`: 命令名
/// - `args`: 命令参数
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
/// - `timeout_secs`: 超时秒数（0 表示无超时）
pub fn exec_captured(
    cmd: &str,
    args: &[String],
    shell_type: &str,
    timeout_secs: u64,
) -> Result<CapturedOutput, NashellError> {
    let mut full_cmd = cmd.to_string();
    for arg in args {
        full_cmd.push(' ');
        full_cmd.push_str(arg);
    }

    let inner_cmd = format!("{} -c {}", shell_type, shell_quote(&full_cmd));

    let child = Command::new("script")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-e")
        .arg("-q")
        .arg("-c")
        .arg(&inner_cmd)
        .arg("/dev/null")
        .env("TERM", "xterm-256color")
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?;

    wait_child_with_timeout(child, &full_cmd, timeout_secs)
}

/// 执行 `cd` 目录切换。
///
/// 由 Rust 进程直接调用 `std::env::set_current_dir()`，
/// 避免 `bash -c cd` 在不同进程中执行导致状态无法持久的问题。
///
/// # 参数
/// - `args`: cd 的参数，`args[0]` 为目标路径。空参数时切换到 home 目录。
pub fn exec_cd(args: &[String]) -> Result<(), NashellError> {
    let target = if args.is_empty() {
        dirs::home_dir().unwrap_or_else(|| "/".into())
    } else {
        let path = &args[0];
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path == "~" {
                    home
                } else if path.starts_with("~/") {
                    home.join(&path[2..])
                } else {
                    std::path::PathBuf::from(path)
                }
            } else {
                std::path::PathBuf::from(path)
            }
        } else {
            std::path::PathBuf::from(path)
        }
    };

    std::env::set_current_dir(&target).map_err(|e| NashellError::Io {
        path: Some(target.display().to_string()),
        source: e,
    })
}

/// 流式捕获执行 shell 命令——实时输出到终端同时收集全部输出。
///
/// 与 `exec_captured` 使用相同的 `script -c` 机制，但不等子进程结束才返回。
/// 而是在后台线程逐块读取 stdout，同步写入终端和缓冲区。
/// 适合 toExec 的单条 Shell 命令：既能实时可见，又能将完整输出返回给插件。
///
/// # 参数
/// - `cmd`: 命令名
/// - `args`: 命令参数
/// - `shell_type`: shell 类型
/// - `timeout_secs`: 超时秒数
pub fn exec_captured_streaming(
    cmd: &str,
    args: &[String],
    shell_type: &str,
    timeout_secs: u64,
) -> Result<CapturedOutput, NashellError> {
    let mut full_cmd = cmd.to_string();
    for arg in args {
        full_cmd.push(' ');
        full_cmd.push_str(arg);
    }

    let inner_cmd = format!("{} -c {}", shell_type, shell_quote(&full_cmd));

    let mut child = Command::new("script")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-e")
        .arg("-q")
        .arg("-c")
        .arg(&inner_cmd)
        .arg("/dev/null")
        .env("TERM", "xterm-256color")
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some("script".to_string()),
            source: e,
        })?;

    let child_stdout = child.stdout.take().ok_or_else(|| NashellError::Io {
        path: Some(cmd.to_string()),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdout pipe missing"),
    })?;
    let child_stderr = child.stderr.take().ok_or_else(|| NashellError::Io {
        path: Some(cmd.to_string()),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stderr pipe missing"),
    })?;

    let child_pid = child.id();
    let label = full_cmd.clone();

    // 读 stdout 线程：逐块写入终端 + 累积缓冲区
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>();
    thread::spawn(move || {
        use std::io::Read;
        let mut reader = std::io::BufReader::new(child_stdout);
        let mut terminal = std::io::stdout();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();
                    let _ = terminal.write_all(&chunk);
                    let _ = terminal.flush();
                    tx.send(Ok(chunk)).ok();
                }
                Err(e) => {
                    tx.send(Err(e)).ok();
                    break;
                }
            }
        }
    });

    // 读 stderr 线程
    let (tx_err, rx_err) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>();
    thread::spawn(move || {
        use std::io::Read;
        let mut reader = std::io::BufReader::new(child_stderr);
        let mut terminal = std::io::stdout();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();
                    let _ = terminal.write_all(&chunk);
                    let _ = terminal.flush();
                    tx_err.send(Ok(chunk)).ok();
                }
                Err(e) => {
                    tx_err.send(Err(e)).ok();
                    break;
                }
            }
        }
    });

    // 收集输出
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let start = std::time::Instant::now();
    let timeout_dur = Duration::from_secs(if timeout_secs > 0 { timeout_secs } else { u64::MAX });

    let mut stdout_done = false;
    let mut stderr_done = false;

    while !stdout_done || !stderr_done {
        if start.elapsed() > timeout_dur {
            let pid = child_pid as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
                thread::sleep(Duration::from_millis(500));
                libc::kill(pid, libc::SIGKILL);
            }
            let _ = child.wait();
            return Err(NashellError::Timeout {
                command: label,
                seconds: timeout_secs,
            });
        }

        if !stdout_done {
            match rx.try_recv() {
                Ok(Ok(chunk)) => stdout_buf.extend_from_slice(&chunk),
                Ok(Err(_)) => stdout_done = true,
                Err(mpsc::TryRecvError::Disconnected) => stdout_done = true,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if !stderr_done {
            match rx_err.try_recv() {
                Ok(Ok(chunk)) => stderr_buf.extend_from_slice(&chunk),
                Ok(Err(_)) => stderr_done = true,
                Err(mpsc::TryRecvError::Disconnected) => stderr_done = true,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        // 检查子进程是否已退出（两个 channel 都断开意味着线程已结束）
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已退出，等待 channel 排空
                // Drain remaining
                while let Ok(Ok(chunk)) = rx.try_recv() {
                    stdout_buf.extend_from_slice(&chunk);
                }
                while let Ok(Ok(chunk)) = rx_err.try_recv() {
                    stderr_buf.extend_from_slice(&chunk);
                }
                let exit_code = status.code().unwrap_or(-1);
                return Ok(CapturedOutput {
                    stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
                    exit_code,
                });
            }
            Ok(None) => {}
            Err(e) => {
                return Err(NashellError::Io {
                    path: Some("script".to_string()),
                    source: e,
                });
            }
        }

        thread::sleep(Duration::from_millis(10));
    }

    // Channel done, wait for process
    let status = child.wait().map_err(|e| NashellError::Io {
        path: Some("script".to_string()),
        source: e,
    })?;
    let exit_code = status.code().unwrap_or(-1);
    Ok(CapturedOutput {
        stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
        stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
        exit_code,
    })
}
///
/// stdin/stdout/stderr 全部继承自父进程，子进程直接读写真实终端。
/// 适用于无管道、无异步、非 Bash 快捷方式的单一命令——
/// 支持实时进度输出和交互式输入（如 `python` REPL、`read` 等）。
///
/// 执行链路：
///   Rust Command → {shell} -c '{command}'  (全部 stdio 继承)
///
/// `cd` 命令不经过此函数，由 Rust 进程直接拦截处理。
///
/// # 参数
/// - `cmd`: 命令名
/// - `args`: 命令参数
/// - `shell_type`: shell 类型（"bash" 或 "nu"）
///
/// # 返回
/// 命令的退出码，-1 表示被信号终止。
pub fn exec_shell_direct(
    cmd: &str,
    args: &[String],
    shell_type: &str,
) -> Result<i32, NashellError> {
    let mut full_cmd = cmd.to_string();
    for arg in args {
        full_cmd.push(' ');
        full_cmd.push_str(arg);
    }

    let status = Command::new(shell_type)
        .arg("-c")
        .arg(&full_cmd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some(cmd.to_string()),
            source: e,
        })?
        .wait()
        .map_err(|e| NashellError::Io {
            path: Some(cmd.to_string()),
            source: e,
        })?;

    Ok(status.code().unwrap_or(-1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_quote_simple() {
        assert_eq!(shell_quote("hello"), "'hello'");
    }

    #[test]
    fn test_shell_quote_with_single_quote() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_exec_captured_basic() {
        let result = exec_captured("echo", &["hello".to_string()], "bash", 120);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.stdout.contains("hello"));
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn test_exec_captured_error() {
        let result =
            exec_captured("ls", &["/nonexistent_path_xyz".to_string()], "bash", 120);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_ne!(output.exit_code, 0);
    }

    #[test]
    fn test_exec_captured_nonexistent_command() {
        let result = exec_captured("nonexistent_command_xyz", &[], "bash", 120);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_ne!(output.exit_code, 0);
        assert!(!output.stderr.is_empty() || output.exit_code == 127);
    }

    #[test]
    fn test_exec_bash_basic() {
        let result = exec_bash("echo hello_from_bash", 120);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.stdout.contains("hello_from_bash"),
            "expected 'hello_from_bash' in stdout, got: '{}'",
            output.stdout
        );
        assert_eq!(output.exit_code, 0);
    }

    #[test]
    fn test_exec_cd_tmp() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&["/tmp".to_string()]);
        assert!(result.is_ok());
        assert_eq!(std::env::current_dir().unwrap(), std::path::PathBuf::from("/tmp"));
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_exec_cd_home() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&["~".to_string()]);
        assert!(result.is_ok());
        let home = dirs::home_dir().unwrap();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&old).ok();
    }

    #[test]
    fn test_exec_cd_empty() {
        let old = std::env::current_dir().unwrap();
        let result = exec_cd(&[]);
        assert!(result.is_ok());
        let home = dirs::home_dir().unwrap();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&old).ok();
    }
}
