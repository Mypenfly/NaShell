use std::path::Path;

/// 根据当前工作路径生成提示符字符串。
///
/// 格式为 `{path} |> `，其中 `{path}` 为当前路径的显示形式。
/// 若路径在 home 目录下，使用 `~` 替换 home 部分。
///
/// # 参数
/// - `cwd`: 当前工作目录路径
/// - `home`: 用户 home 目录路径（用于 `~` 缩写）
///
/// # 返回
/// 提示符字符串，如 `~/projects/nashell |> `
///
/// # 示例
/// ```
/// let prompt = generate_prompt("/home/user/projects", "/home/user");
/// assert_eq!(prompt, "~/projects |> ");
/// ```
pub fn generate_prompt(cwd: &Path, home: Option<&Path>) -> String {
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

    format!("{} |> ", path_display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_prompt_under_home() {
        let cwd = PathBuf::from("/home/user/projects/nashell");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home);
        assert_eq!(prompt, "~/projects/nashell |> ");
    }

    #[test]
    fn test_prompt_exactly_home() {
        let cwd = PathBuf::from("/home/user");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home);
        assert_eq!(prompt, "~ |> ");
    }

    #[test]
    fn test_prompt_not_under_home() {
        let cwd = PathBuf::from("/tmp/test");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home);
        assert_eq!(prompt, "/tmp/test |> ");
    }

    #[test]
    fn test_prompt_no_home() {
        let cwd = PathBuf::from("/home/user/projects");
        let prompt = generate_prompt(&cwd, None);
        assert_eq!(prompt, "/home/user/projects |> ");
    }

    #[test]
    fn test_prompt_ends_with_pipe_arrow() {
        let cwd = PathBuf::from("/tmp");
        let home = Some(Path::new("/home/user"));
        let prompt = generate_prompt(&cwd, home);
        assert!(prompt.ends_with("|> "));
    }
}
