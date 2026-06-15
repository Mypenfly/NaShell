use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 是否收到 SIGINT (Ctrl+C) 中断信号。
static INTERRUPT_FLAG: AtomicBool = AtomicBool::new(false);
/// 是否收到强制退出信号（SIGTERM/SIGHUP 或双次 Ctrl+C）。
static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);
/// 上次 SIGINT 的时间戳（毫秒），用于检测双次 Ctrl+C。
static LAST_SIGINT_MS: AtomicI64 = AtomicI64::new(0);
/// 双次 Ctrl+C 的最大间隔（毫秒）。
const DOUBLE_CTRLC_INTERVAL_MS: i64 = 500;

/// 安装所有信号处理器。
///
/// 注册 SIGINT、SIGTERM、SIGHUP 的信号处理函数。
/// SIGWINCH 被显式忽略以保持默认行为（终端驱动自动更新窗口大小）。
///
/// # Safety
/// 此函数使用 `libc::signal` 注册信号处理器。处理器内部仅操作
/// `AtomicBool` 和 `AtomicI64`（均为 lock-free 类型），符合
/// POSIX 异步信号安全要求。
pub fn install_handlers() {
    unsafe {
        // SIGINT (Ctrl+C) — 设置中断标志
        libc::signal(libc::SIGINT, handle_sigint as *const () as libc::sighandler_t);
        // SIGTERM — 设置优雅退出标志
        libc::signal(libc::SIGTERM, handle_shutdown as *const () as libc::sighandler_t);
        // SIGHUP — 设置优雅退出标志
        libc::signal(libc::SIGHUP, handle_shutdown as *const () as libc::sighandler_t);
        // SIGWINCH — 忽略，终端驱动自动处理
        libc::signal(libc::SIGWINCH, libc::SIG_IGN);
    }
}

/// SIGINT 信号处理器。
///
/// 检测双次 Ctrl+C：若两次 SIGINT 间隔小于 500ms，视为强制退出。
/// 否则设置中断标志，主循环检测后中断当前命令并回到提示符。
unsafe extern "C" fn handle_sigint(_sig: i32) {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let last_ms = LAST_SIGINT_MS.load(Ordering::Relaxed);

    if last_ms > 0 && (now_ms - last_ms) < DOUBLE_CTRLC_INTERVAL_MS {
        // 双次 Ctrl+C：强制退出
        SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    } else {
        // 首次 Ctrl+C：中断当前操作
        INTERRUPT_FLAG.store(true, Ordering::SeqCst);
    }

    LAST_SIGINT_MS.store(now_ms, Ordering::Relaxed);
}

/// SIGTERM / SIGHUP 信号处理器。
unsafe extern "C" fn handle_shutdown(_sig: i32) {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
}

/// 检查并清除中断标志。
///
/// 若自上次检查后收到过 SIGINT，返回 true 并清除标志。
/// 用于 REPL 循环中检测是否应中断当前命令。
pub fn check_and_clear_interrupt() -> bool {
    INTERRUPT_FLAG.swap(false, Ordering::SeqCst)
}

/// 检查是否需要关闭程序。
///
/// 若收到 SIGTERM、SIGHUP 或双次 Ctrl+C，返回 true。
pub fn should_shutdown() -> bool {
    SHUTDOWN_FLAG.load(Ordering::SeqCst)
}
