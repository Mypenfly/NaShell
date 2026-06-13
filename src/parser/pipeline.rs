use crate::error::NashellError;

/// 按管道符号 `|` 分割命令语句，保护引号内的 `|`。
///
/// 单引号和双引号内的 `|` 不会触发分割。
///
/// # 参数
/// - `cmd_part`: 待分割的命令语句字符串
///
/// # 返回
/// `Result<Vec<String>, NashellError>` — 分割后的命令段列表，去除首尾空格
pub fn split_pipeline(cmd_part: &str) -> Result<Vec<String>, NashellError> {
    if cmd_part.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = cmd_part.chars().collect();
    let len = chars.len();
    let mut start = 0;
    let mut i = 0;

    while i < len {
        match chars[i] {
            '"' | '\'' => {
                let quote = chars[i];
                i += 1; // 跳过开引号
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 1; // 跳过转义符
                    }
                    i += 1;
                }
                if i < len {
                    i += 1; // 跳过闭引号
                }
            }
            '|' => {
                let segment = cmd_part[start..i].trim().to_string();
                segments.push(segment);
                i += 1;
                start = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    // 最后一段
    let last = cmd_part[start..].trim().to_string();
    segments.push(last);

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_pipeline_single_command() {
        let result = split_pipeline("ls -la").unwrap();
        assert_eq!(result, vec!["ls -la".to_string()]);
    }

    #[test]
    fn test_split_pipeline_two_commands() {
        let result = split_pipeline("ls -la | grep foo").unwrap();
        assert_eq!(result, vec!["ls -la".to_string(), "grep foo".to_string()]);
    }

    #[test]
    fn test_split_pipeline_three_commands() {
        let result = split_pipeline("cat file | grep foo | wc -l").unwrap();
        assert_eq!(result, vec![
            "cat file".to_string(),
            "grep foo".to_string(),
            "wc -l".to_string(),
        ]);
    }

    #[test]
    fn test_split_pipeline_pipe_in_double_quotes() {
        let result = split_pipeline("echo \"hello | world\"").unwrap();
        assert_eq!(result, vec!["echo \"hello | world\"".to_string()]);
    }

    #[test]
    fn test_split_pipeline_pipe_in_single_quotes() {
        let result = split_pipeline("echo 'hello | world'").unwrap();
        assert_eq!(result, vec!["echo 'hello | world'".to_string()]);
    }

    #[test]
    fn test_split_pipeline_mixed_shell_and_nacommand() {
        let result = split_pipeline("ls -la | !@Write:./out.txt").unwrap();
        assert_eq!(result, vec![
            "ls -la".to_string(),
            "!@Write:./out.txt".to_string(),
        ]);
    }

    #[test]
    fn test_split_pipeline_no_pipe() {
        let result = split_pipeline("echo hello").unwrap();
        assert_eq!(result, vec!["echo hello".to_string()]);
    }

    #[test]
    fn test_split_pipeline_empty() {
        let result = split_pipeline("").unwrap();
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_split_pipeline_pipe_with_spaces() {
        let result = split_pipeline("ls   |   grep foo").unwrap();
        assert_eq!(result, vec!["ls".to_string(), "grep foo".to_string()]);
    }

    #[test]
    fn test_split_pipeline_leading_trailing_pipe() {
        let result = split_pipeline("| ls").unwrap();
        assert_eq!(result, vec!["".to_string(), "ls".to_string()]);
    }
}
