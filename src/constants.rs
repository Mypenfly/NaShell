/// 文件读取默认行数限制
pub const DEFAULT_OPEN_LIMIT: usize = 500;

/// 文件读取最大行数
pub const MAX_OPEN_LIMIT: usize = 2000;

/// Shell 默认超时（秒）
pub const DEFAULT_SHELL_TIMEOUT_SECS: u64 = 120;

/// 插件通信默认超时（秒）
pub const PLUGIN_TIMEOUT_SECS: u64 = 30;

/// toExec 最大递归深度
pub const TOEXEC_MAX_DEPTH: u32 = 3;

/// 管道输出默认最大字节数（不截断时为 0）
pub const MAX_PIPE_BUFFER_BYTES: usize = 0;

/// PTY 窗口默认列数
pub const DEFAULT_PTY_COLS: u16 = 80;

/// PTY 窗口默认行数
pub const DEFAULT_PTY_ROWS: u16 = 24;

/// cwd 轮询间隔（毫秒）
pub const CWD_POLL_INTERVAL_MS: u64 = 200;

/// 配置文件默认路径（相对于 home）
pub const DEFAULT_CONFIG_PATH: &str = ".config/nashell/config.kdl";

/// 插件默认目录（相对于 home）
pub const DEFAULT_PLUGINS_DIR: &str = ".config/nashell/plugins";

#[cfg(test)]
mod tests {
    #[test]
    fn test_all_constants() {
        assert_eq!(super::DEFAULT_OPEN_LIMIT, 500);
        assert_eq!(super::MAX_OPEN_LIMIT, 2000);
        assert_eq!(super::DEFAULT_SHELL_TIMEOUT_SECS, 120);
        assert_eq!(super::PLUGIN_TIMEOUT_SECS, 30);
        assert_eq!(super::TOEXEC_MAX_DEPTH, 3);
        assert_eq!(super::MAX_PIPE_BUFFER_BYTES, 0);
        assert_eq!(super::DEFAULT_PTY_COLS, 80);
        assert_eq!(super::DEFAULT_PTY_ROWS, 24);
        assert_eq!(super::CWD_POLL_INTERVAL_MS, 200);
        assert_eq!(super::DEFAULT_CONFIG_PATH, ".config/nashell/config.kdl");
        assert_eq!(super::DEFAULT_PLUGINS_DIR, ".config/nashell/plugins");
    }
}
