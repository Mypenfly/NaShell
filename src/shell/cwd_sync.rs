use std::path::PathBuf;

use crate::error::NashellError;

/// 通过进程 PID 同步当前工作目录。
///
/// 读取 Linux 的 `/proc/{pid}/cwd` 符号链接获取进程的当前工作目录。
/// 在其他平台上可能返回错误。
///
/// # 参数
/// - `pid`: 目标进程的 PID
///
/// # 返回
/// 进程当前工作目录的绝对路径。
///
/// # 错误
/// - 进程不存在
/// - 无权读取 /proc 文件系统
pub fn sync_cwd_by_pid(pid: u32) -> Result<PathBuf, NashellError> {
    let proc_path = format!("/proc/{}/cwd", pid);
    std::fs::read_link(&proc_path).map_err(|e| NashellError::Io {
        path: Some(proc_path),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_cwd_self() {
        let pid = std::process::id();
        let result = sync_cwd_by_pid(pid);
        assert!(result.is_ok(), "sync_cwd should succeed for own process");
        let cwd = result.unwrap();
        assert!(cwd.is_absolute(), "cwd should be absolute path");
    }

    #[test]
    fn test_sync_cwd_invalid_pid() {
        let result = sync_cwd_by_pid(0);
        assert!(result.is_err());
    }
}
