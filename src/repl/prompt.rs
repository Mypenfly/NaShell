use std::path::Path;

/// 将颜色名称映射为 ANSI 前景色转义码。
///
/// # 参数
/// - `color`: 颜色名称，如 `"green"`、`"bright_yellow"`
///
/// # 返回
/// ANSI 转义码字符串，如 `"\x1b[32m"`。未知颜色返回空字符串。
pub fn ansi_code(color: &str) -> &str {
    match color {
        "black" => "\x1b[30m",
        "red" => "\x1b[31m",
        "green" => "\x1b[32m",
        "yellow" => "\x1b[33m",
        "blue" => "\x1b[34m",
        "magenta" => "\x1b[35m",
        "cyan" => "\x1b[36m",
        "white" => "\x1b[37m",
        "bright_black" | "grey" | "gray" => "\x1b[90m",
        "bright_red" => "\x1b[91m",
        "bright_green" => "\x1b[92m",
        "bright_yellow" => "\x1b[93m",
        "bright_blue" => "\x1b[94m",
        "bright_magenta" => "\x1b[95m",
        "bright_cyan" => "\x1b[96m",
        "bright_white" => "\x1b[97m",
        _ => "",
    }
}

/// ANSI 重置码。
const ANSI_RESET: &str = "\x1b[0m";

/// 为文本包裹 ANSI 前景色。
///
/// # 参数
/// - `text`: 原始文本
/// - `color`: 颜色名称
pub fn colorize(text: &str, color: &str) -> String {
    let code = ansi_code(color);
    if code.is_empty() {
        text.to_string()
    } else {
        format!("{}{}{}", code, text, ANSI_RESET)
    }
}

/// 根据当前工作路径生成提示符字符串。
///
/// 使用配置中的格式模板（如 `{path} |> `），将 `{path}` 替换为
/// 当前路径的显示形式。若路径在 home 目录下，使用 `~` 替换 home 部分。
///
/// # 参数
/// - `cwd`: 当前工作目录路径
/// - `home`: 用户 home 目录路径（用于 `~` 缩写）
/// - `format`: 提示符格式模板，用 `{path}` 表示路径占位符
///
/// # 返回
/// 提示符字符串，如 `~/projects/nashell |> `
///
/// # 示例
/// ```
/// use std::path::Path;
/// let prompt = generate_prompt(
///     Path::new("/home/user/projects"),
///     Some(Path::new("/home/user")),
///     "{path} |> ",
/// );
/// assert_eq!(prompt, "~/projects |> ");
/// ```
pub fn generate_prompt(cwd: &Path, home: Option<&Path>, format: &str) -> String {
    let path_display = match home {
        Some(home_path) => {
            match cwd.strip_prefix(home_path) {
                Ok(relative) => {
                    if relative.as_os_str().is_empty() {
                        "~".to_string()
                    } else {
                        format!("~/{}", relative.display())
                    }
                }
                Err(_) => cwd.display().to_string(),
            }
        }
        None => cwd.display().to_string(),
    };

    format.replace("{path}", &path_display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FMT: &str = "{path} |> ";

    #[test]
    fn test_prompt_under_home() {
        let cwd = PathBuf::from("/home/user/projects/nashell");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home, FMT);
        assert_eq!(prompt, "~/projects/nashell |> ");
    }

    #[test]
    fn test_prompt_exactly_home() {
        let cwd = PathBuf::from("/home/user");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home, FMT);
        assert_eq!(prompt, "~ |> ");
    }

    #[test]
    fn test_prompt_not_under_home() {
        let cwd = PathBuf::from("/tmp/test");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home, FMT);
        assert_eq!(prompt, "/tmp/test |> ");
    }

    #[test]
    fn test_prompt_no_home() {
        let cwd = PathBuf::from("/home/user/projects");
        let prompt = generate_prompt(&cwd, None, FMT);
        assert_eq!(prompt, "/home/user/projects |> ");
    }

    #[test]
    fn test_prompt_ends_with_pipe_arrow() {
        let cwd = PathBuf::from("/tmp");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home, FMT);
        assert!(prompt.ends_with("|> "));
    }

    #[test]
    fn test_prompt_custom_format() {
        let cwd = PathBuf::from("/tmp");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home, "[{path}] $ ");
        assert_eq!(prompt, "[/tmp] $ ");
    }
}
