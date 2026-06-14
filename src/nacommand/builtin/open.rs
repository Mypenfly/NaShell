use crate::constants::{DEFAULT_OPEN_LIMIT, MAX_OPEN_LIMIT};
use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// 解析 Open 命令的可选参数。
///
/// 从 args 中提取 --limit/-l, --start/-s, --end/-e。
/// 返回 (limit, start, end) 的元组。
/// start 和 end 是 1-indexed 的。
fn parse_open_options(args: &[String]) -> Result<(usize, usize, Option<usize>), NashellError> {
    let mut limit: usize = DEFAULT_OPEN_LIMIT;
    let mut start: usize = 1;
    let mut end: Option<usize> = None;

    let mut i = 1; // start after path (args[0])
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-l" | "--limit" => {
                if i + 1 >= args.len() {
                    return Err(NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("{} 缺少值", arg),
                    });
                }
                limit = args[i + 1].parse::<usize>().map_err(|_| {
                    NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!(
                            "无效的 {} 值: {}",
                            arg, args[i + 1]
                        ),
                    }
                })?;
                i += 2;
            }
            "-s" | "--start" => {
                if i + 1 >= args.len() {
                    return Err(NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("{} 缺少值", arg),
                    });
                }
                start = args[i + 1].parse::<usize>().map_err(|_| {
                    NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!(
                            "无效的 {} 值: {}",
                            arg, args[i + 1]
                        ),
                    }
                })?;
                i += 2;
            }
            "-e" | "--end" => {
                if i + 1 >= args.len() {
                    return Err(NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("{} 缺少值", arg),
                    });
                }
                end = Some(args[i + 1].parse::<usize>().map_err(|_| {
                    NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!(
                            "无效的 {} 值: {}",
                            arg, args[i + 1]
                        ),
                    }
                })?);
                i += 2;
            }
            _ => {
                // Ignore unknown flags — they might be positional or mode-related
                i += 1;
            }
        }
    }

    if limit > MAX_OPEN_LIMIT {
        limit = MAX_OPEN_LIMIT;
    }

    Ok((limit, start, end))
}

/// 检查 args 中除 path (index 0) 外是否包含文件选项。
fn has_file_options(args: &[String]) -> bool {
    if args.len() <= 1 {
        return false;
    }
    for i in 1..args.len() {
        let arg = args[i].as_str();
        if arg == "-l" || arg == "--limit" || arg == "-s" || arg == "--start" || arg == "-e" || arg == "--end" {
            return true;
        }
    }
    false
}

/// 生成目录结构树。
///
/// 递归遍历目录，返回类似 tree 命令的输出。
fn generate_dir_tree(path: &PathBuf, prefix: &str, is_last: bool) -> String {
    let mut output = String::new();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let connector = if is_last { "└── " } else { "├── " };

    if prefix.is_empty() {
        output.push_str(&format!("{}\n", name));
    } else {
        output.push_str(&format!("{}{}{}\n", prefix, connector, name));
    }

    let mut entries: Vec<_> = match fs::read_dir(path) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return output,
    };
    entries.sort_by_key(|e| e.file_name());

    let len = entries.len();
    for (idx, entry) in entries.into_iter().enumerate() {
        let child_path = entry.path();
        let is_child_last = idx == len - 1;
        let child_prefix = if prefix.is_empty() {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        let child_name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            output.push_str(&generate_dir_tree(&child_path, &child_prefix, is_child_last));
        } else {
            let connector = if is_child_last { "└── " } else { "├── " };
            output.push_str(&format!("{}{}{}\n", child_prefix, connector, child_name));
        }
    }

    output
}

/// 读取文件内容并以带行号格式返回。
fn read_file_with_options(
    path: &PathBuf,
    limit: usize,
    start: usize,
    end: Option<usize>,
) -> Result<String, NashellError> {
    let file = fs::File::open(path).map_err(|e| NashellError::Io {
        path: Some(path.display().to_string()),
        source: e,
    })?;
    let reader = BufReader::new(file);

    let mut output = String::new();
    let max_line = end.unwrap_or(start + limit - 1);
    if start < 1 {
        return Err(NashellError::Execute {
            command: "open".to_string(),
            exit_code: None,
            stderr: format!("起始行号 {} 无效，必须 >= 1", start),
        });
    }

    for (idx, line) in reader.lines().enumerate() {
        let line_num = idx + 1; // 1-indexed
        if line_num < start {
            continue;
        }
        if line_num > max_line {
            break;
        }
        let line_content = line.map_err(|e| NashellError::Io {
            path: Some(path.display().to_string()),
            source: e,
        })?;
        let width = max_line.to_string().len();
        output.push_str(&format!("{:>width$}  {}\n", line_num, line_content, width = width));
    }

    Ok(output)
}

/// 执行 Open 命令：打开文件或文件夹。
///
/// - path 为目录：输出目录结构树（类似 tree 命令）。
/// - path 为文件：输出带行号的文件内容，支持 --limit/-l, --start/-s, --end/-e。
/// - path 为目录时传入文件选项参数报错。
///
/// # 参数
/// - `cmd`: NaCommand 数据结构
///
/// # 返回
/// - `Ok(String)`: 格式化后的内容
/// - `Err(NashellError)`: 路径不存在、参数错误或 IO 错误
pub fn execute_open(cmd: &NaCommand) -> Result<String, NashellError> {
    let path_str = cmd.args.first().ok_or_else(|| NashellError::Execute {
        command: "open".to_string(),
        exit_code: None,
        stderr: "缺少路径参数".to_string(),
    })?;

    let file_path = PathBuf::from(path_str);

    if !file_path.exists() {
        return Err(NashellError::Execute {
            command: "open".to_string(),
            exit_code: None,
            stderr: format!("路径不存在: {}", file_path.display()),
        });
    }

    if file_path.is_dir() {
        if has_file_options(&cmd.args) {
            return Err(NashellError::Execute {
                command: "open".to_string(),
                exit_code: None,
                stderr: "目录模式下不支持 --limit/--start/--end 参数".to_string(),
            });
        }
        let tree = generate_dir_tree(&file_path, "", true);
        Ok(tree)
    } else {
        let (limit, start, end) = parse_open_options(&cmd.args)?;
        read_file_with_options(&file_path, limit, start, end)
    }
}

#[cfg(test)]
mod tests {
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

        let result = execute_open(&cmd).unwrap();
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

        let result = execute_open(&cmd).unwrap();
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

        let result = execute_open(&cmd).unwrap();
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

        let result = execute_open(&cmd).unwrap();
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

        let result = execute_open(&cmd).unwrap();
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
    fn test_open_directory_with_file_options_errors() {
        let dir = setup_temp_dir();
        let test_dir = create_test_dir(&dir, "mydir");

        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "open".to_string(),
            mode: None,
            args: vec![
                test_dir.to_string_lossy().to_string(),
                "-l".to_string(),
                "10".to_string(),
            ],
            long_argument: None,
        };

        let result = execute_open(&cmd);
        assert!(result.is_err());
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
}
