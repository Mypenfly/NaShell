use super::*;
use crate::nacommand::cmd::{NaCommand, NaLevel};
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn setup_temp_dir() -> std::path::PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("nashell_open_{}_{}", std::process::id(), id));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_test_file(dir: &std::path::Path, name: &str, lines: usize) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    for i in 1..=lines {
        writeln!(f, "line {}", i).unwrap();
    }
    path
}

fn create_test_dir(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("a.txt"), "hello").unwrap();
    std::fs::create_dir_all(path.join("subdir")).unwrap();
    std::fs::write(path.join("subdir").join("b.txt"), "world").unwrap();
    path
}

// --- File tests ---

/// 移除 ANSI 转义码，用于测试断言（语法高亮会产生 ANSI 码）。
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_digit() || nc == ';' {
                    chars.next();
                } else {
                    break;
                }
            }
            chars.next();
            continue;
        }
        result.push(c);
    }
    result
}

#[test]
fn test_open_file_default_limit() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![file_path.to_string_lossy().to_string()],
        long_argument: None,
    };

    let result = strip_ansi(&execute_open(&cmd).unwrap());
    assert!(result.contains("1  line 1"));
    assert!(result.contains("10  line 10"));
    assert!(!result.contains("11  "));
}

#[test]
fn test_open_file_with_limit() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            file_path.to_string_lossy().to_string(),
            "-l".to_string(),
            "3".to_string(),
        ],
        long_argument: None,
    };

    let result = strip_ansi(&execute_open(&cmd).unwrap());
    assert!(result.contains("1  line 1"));
    assert!(result.contains("3  line 3"));
    assert!(!result.contains("4  line 4"));
}

#[test]
fn test_open_file_with_start() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            file_path.to_string_lossy().to_string(),
            "-s".to_string(),
            "5".to_string(),
        ],
        long_argument: None,
    };

    let result = strip_ansi(&execute_open(&cmd).unwrap());
    assert!(!result.contains("4  line 4"));
    assert!(result.contains("5  line 5"));
}

#[test]
fn test_open_file_with_end() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            file_path.to_string_lossy().to_string(),
            "-e".to_string(),
            "3".to_string(),
        ],
        long_argument: None,
    };

    let result = strip_ansi(&execute_open(&cmd).unwrap());
    assert!(result.contains("1  line 1"));
    assert!(result.contains("3  line 3"));
    assert!(!result.contains("4  line 4"));
}

#[test]
fn test_open_file_with_long_flag() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            file_path.to_string_lossy().to_string(),
            "--limit".to_string(),
            "2".to_string(),
        ],
        long_argument: None,
    };

    let result = strip_ansi(&execute_open(&cmd).unwrap());
    assert!(result.contains("1  line 1"));
    assert!(result.contains("2  line 2"));
    assert!(!result.contains("3  line 3"));
}

#[test]
fn test_open_file_nonexistent() {
    let dir = setup_temp_dir();
    let file_path = dir.join("nonexistent.txt");

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![file_path.to_string_lossy().to_string()],
        long_argument: None,
    };

    let result = execute_open(&cmd);
    assert!(result.is_err());
}

#[test]
fn test_open_no_path_argument() {
    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![],
        long_argument: None,
    };

    let result = execute_open(&cmd);
    assert!(result.is_err());
}

// --- Directory tests ---

#[test]
fn test_open_directory() {
    let dir = setup_temp_dir();
    let test_dir = create_test_dir(&dir, "mydir");

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![test_dir.to_string_lossy().to_string()],
        long_argument: None,
    };

    let result = execute_open(&cmd).unwrap();
    assert!(result.contains("a.txt"));
    assert!(result.contains("subdir"));
    assert!(result.contains("b.txt"));
}

#[test]
fn test_open_directory_with_depth_limit() {
    let dir = setup_temp_dir();
    let test_dir = create_test_dir(&dir, "mydir");

    // 默认深度为 3，子目录内容应可见
    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![test_dir.to_string_lossy().to_string()],
        long_argument: None,
    };

    let result = execute_open(&cmd).unwrap();
    assert!(result.contains("subdir"));
    assert!(result.contains("b.txt"));

    // 深度 1：只显示根目录内容，不展开子目录
    let cmd_shallow = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            test_dir.to_string_lossy().to_string(),
            "-l".to_string(),
            "1".to_string(),
        ],
        long_argument: None,
    };

    let result_shallow = execute_open(&cmd_shallow).unwrap();
    assert!(result_shallow.contains("a.txt"));
    assert!(!result_shallow.contains("b.txt"));
}

// --- Invalid options ---

#[test]
fn test_open_invalid_limit() {
    let dir = setup_temp_dir();
    let file_path = create_test_file(&dir, "test.txt", 10);

    let cmd = NaCommand {
        level: NaLevel::Normal,
        cmd: "open".to_string(),
        mode: None,
        args: vec![
            file_path.to_string_lossy().to_string(),
            "-l".to_string(),
            "not_a_number".to_string(),
        ],
        long_argument: None,
    };

    let result = execute_open(&cmd);
    assert!(result.is_err());
}
