use crate::constants::{DEFAULT_OPEN_DIR_DEPTH, DEFAULT_OPEN_LIMIT, MAX_OPEN_LIMIT};
use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static syntect::highlighting::Theme {
    static THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes["base16-ocean.dark"].clone()
    })
}

/// 高亮单行内容。
///
/// 根据文件扩展名选择语法，返回带 ANSI 转义码的彩色行。
/// 高亮器 `h` 必须按行顺序调用以维护内部解析状态。
///
/// # 参数
/// - `h`: 行高亮器（可复用，调用方管理其生命周期）
/// - `ss`: 语法定义集
/// - `line`: 原始行文本（不含换行符）
///
/// # 返回
/// ANSI 高亮后的行文本
fn highlight_line(h: &mut HighlightLines, ss: &SyntaxSet, line: &str) -> String {
    let ranges = h.highlight_line(line, ss).unwrap_or_default();
    as_24_bit_terminal_escaped(&ranges[..], false)
}

/// 根据文件扩展名创建高亮器。
///
/// 若找不到对应的语法定义，回退到纯文本。
fn new_highlighter(extension: &str) -> (HighlightLines<'static>, &'static SyntaxSet) {
    let ss = syntax_set();
    let syntax = ss
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let h = HighlightLines::new(syntax, theme());
    (h, ss)
}

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
                limit = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("无效的 {} 值: {}", arg, args[i + 1]),
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
                start = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("无效的 {} 值: {}", arg, args[i + 1]),
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
                end = Some(
                    args[i + 1]
                        .parse::<usize>()
                        .map_err(|_| NashellError::Execute {
                            command: "open".to_string(),
                            exit_code: None,
                            stderr: format!("无效的 {} 值: {}", arg, args[i + 1]),
                        })?,
                );
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

/// 从 args 中提取目录递归深度（仅 --limit/-l）。
///
/// 默认深度为 DEFAULT_OPEN_DIR_DEPTH（3）。
fn parse_dir_depth(args: &[String]) -> Result<usize, NashellError> {
    let mut depth = DEFAULT_OPEN_DIR_DEPTH;
    let mut i = 1;
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
                depth = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| NashellError::Execute {
                        command: "open".to_string(),
                        exit_code: None,
                        stderr: format!("无效的 {} 值: {}", arg, args[i + 1]),
                    })?;
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
    Ok(depth)
}

/// 检查 args 中除 path (index 0) 外是否包含仅文件可用的选项（-s/-e）。
///
/// --limit/-l 已同时支持文件和目录（文件=行数，目录=深度），不在检查范围内。
fn has_file_only_options(args: &[String]) -> bool {
    args.iter()
        .skip(1)
        .any(|a| matches!(a.as_str(), "-s" | "--start" | "-e" | "--end"))
}

/// 生成目录结构树。
///
/// 递归遍历目录，返回类似 tree 命令的输出。
/// `depth` 为当前递归深度（首次调用传 1），`max_depth` 为上限。
fn generate_dir_tree(
    path: &PathBuf,
    prefix: &str,
    is_last: bool,
    depth: usize,
    max_depth: usize,
) -> String {
    let mut output = String::new();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if depth > max_depth {
        return output;
    }

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
            if depth < max_depth {
                // 可递归进入子目录
                output.push_str(&generate_dir_tree(
                    &child_path,
                    &child_prefix,
                    is_child_last,
                    depth + 1,
                    max_depth,
                ));
            } else {
                // 深度已达上限，仅打印目录名，不再展开
                let connector = if is_child_last {
                    "└── "
                } else {
                    "├── "
                };
                output.push_str(&format!("{}{}{}\n", child_prefix, connector, child_name));
            }
        } else {
            let connector = if is_child_last {
                "└── "
            } else {
                "├── "
            };
            output.push_str(&format!("{}{}{}\n", child_prefix, connector, child_name));
        }
    }

    output
}

/// 读取文件内容并以带行号、语法高亮的格式返回。
fn read_file_with_options(
    path: &PathBuf,
    limit: usize,
    start: usize,
    end: Option<usize>,
) -> Result<String, NashellError> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let (mut h, ss) = new_highlighter(extension);

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

    let width = max_line.to_string().len();

    for (idx, line) in reader.lines().enumerate() {
        let line_num = idx + 1;
        if line_num < start {
            // 仍然推进高亮器状态以保持上下文正确
            let line_content = line.map_err(|e| NashellError::Io {
                path: Some(path.display().to_string()),
                source: e,
            })?;
            let _ = h.highlight_line(&line_content, ss);
            continue;
        }
        if line_num > max_line {
            break;
        }
        let line_content = line.map_err(|e| NashellError::Io {
            path: Some(path.display().to_string()),
            source: e,
        })?;
        let highlighted = highlight_line(&mut h, ss, &line_content);
        output.push_str(&format!(
            "{:>width$}  {}\n",
            line_num,
            highlighted,
            width = width
        ));
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
        if has_file_only_options(&cmd.args) {
            return Err(NashellError::Execute {
                command: "open".to_string(),
                exit_code: None,
                stderr: "目录模式下不支持 --start/--end 参数，--limit/-l 控制递归深度".to_string(),
            });
        }
        let max_depth = parse_dir_depth(&cmd.args)?;
        let tree = generate_dir_tree(&file_path, "", true, 1, max_depth);
        Ok(tree)
    } else {
        let (limit, start, end) = parse_open_options(&cmd.args)?;
        read_file_with_options(&file_path, limit, start, end)
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
