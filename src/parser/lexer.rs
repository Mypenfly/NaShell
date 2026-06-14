use crate::error::NashellError;

/// 词法分析阶段的 Token 类型。
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// @/ 截止符
    Terminator,
    /// @/Async(name) 异步标记
    AsyncMarker(String),
    /// !!@ 系统级 NaCommand 前缀
    SystemPrefix,
    /// !@ 普通 NaCommand 前缀
    NormalPrefix,
    /// 管道符号 |
    Pipe,
    /// 词（命令名、参数、值）
    Word(String),
}

/// 检测 `!!@Bash:` 快捷方式。
///
/// 若输入的首个非空格词为 `!!@Bash:`，返回其后至首个 `@/` 或空行之前的内容。
/// 否则返回 None。
///
/// # 参数
/// - `input`: 用户输入的完整字符串
///
/// # 返回
/// `Option<String>` — bash 命令的参数部分
pub fn detect_bash_shortcut(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    let prefix = "!!@Bash:";
    if !trimmed.starts_with(prefix) {
        return None;
    }
    let after_prefix = trimmed[prefix.len()..].trim_start();
    // 仅取首行（至换行符）
    let first_line = after_prefix.split('\n').next().unwrap_or(after_prefix);
    // 查找 @/ 截止符
    let args = if let Some(pos) = first_line.find("@/") {
        first_line[..pos].trim().to_string()
    } else {
        first_line.trim().to_string()
    };
    Some(args)
}

/// 检测首行末尾的 `@/Async(name)` 异步标记。
///
/// # 参数
/// - `first_line`: 输入的首行字符串
///
/// # 返回
/// `Option<String>` — 异步 shell 的名称，若无则返回 None
pub fn detect_async_marker(first_line: &str) -> Option<String> {
    let line = first_line.trim();
    if !line.ends_with(')') {
        return None;
    }
    let marker = "@/Async(";
    if let Some(pos) = line.rfind(marker) {
        let after = &line[pos + marker.len()..];
        if let Some(end) = after.find(')') {
            let name = after[..end].to_string();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let rest = &line[pos + marker.len() + end + 1..];
                if rest.trim().is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// 将输入字符串转为 Token 流。
///
/// 正确处理引号内的管道 `|` 和截止符 `@/`（不被识别为分隔符）。
///
/// # 参数
/// - `input`: 待解析的输入字符串
///
/// # 返回
/// `Result<Vec<Token>, NashellError>` — Token 列表或解析错误
///
/// # 错误
/// 引号未闭合时返回 `NashellError::Parse`
pub fn tokenize(input: &str) -> Result<Vec<Token>, NashellError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // 跳过空白
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // 双引号字符串
        if c == '"' {
            let mut s = String::new();
            i += 1; // 跳过开引号
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1; // 跳过转义符
                    s.push(chars[i]);
                } else {
                    s.push(chars[i]);
                }
                i += 1;
            }
            if i >= len {
                return Err(NashellError::Parse {
                    context: input.to_string(),
                    detail: "缺少闭合引号".to_string(),
                });
            }
            i += 1; // 跳过闭引号
            tokens.push(Token::Word(s));
            continue;
        }

        // 单引号字符串
        if c == '\'' {
            let mut s = String::new();
            i += 1; // 跳过开引号
            while i < len && chars[i] != '\'' {
                s.push(chars[i]);
                i += 1;
            }
            if i >= len {
                return Err(NashellError::Parse {
                    context: input.to_string(),
                    detail: "缺少闭合引号".to_string(),
                });
            }
            i += 1; // 跳过闭引号
            tokens.push(Token::Word(s));
            continue;
        }

        // 检查 !!@ 前缀（仅在段开头作为命令类型前缀）
        if tokens.is_empty() && i + 2 < len && chars[i] == '!' && chars[i + 1] == '!' && chars[i + 2] == '@' {
            tokens.push(Token::SystemPrefix);
            i += 3;
            // 读取命令名（直到 : 或空白）
            let mut cmd = String::new();
            while i < len && chars[i] != ':' && !chars[i].is_whitespace() {
                cmd.push(chars[i]);
                i += 1;
            }
            if !cmd.is_empty() {
                tokens.push(Token::Word(cmd));
            }
            // 跳过 :
            if i < len && chars[i] == ':' {
                i += 1;
            }
            continue;
        }

        // 检查 !@ 前缀（仅在段开头作为命令类型前缀）
        if tokens.is_empty() && i + 1 < len && chars[i] == '!' && chars[i + 1] == '@' {
            tokens.push(Token::NormalPrefix);
            i += 2;
            let mut cmd = String::new();
            while i < len && chars[i] != ':' && !chars[i].is_whitespace() {
                cmd.push(chars[i]);
                i += 1;
            }
            if !cmd.is_empty() {
                tokens.push(Token::Word(cmd));
            }
            if i < len && chars[i] == ':' {
                i += 1;
            }
            continue;
        }

        // 检查 @/ 截止符和 @/Async(name)
        if c == '@' && i + 1 < len && chars[i + 1] == '/' {
            let rest = &input[i..];
            if let Some(name) = detect_async_marker(rest) {
                let name_len = name.len();
                tokens.push(Token::AsyncMarker(name));
                // 跳过整个 @/Async(name)
                let marker = "@/Async(";
                i += marker.len() + name_len + 1; // +1 for )
            } else {
                tokens.push(Token::Terminator);
                i += 2; // 跳过 @/
            }
            continue;
        }

        // 管道符号
        if c == '|' {
            tokens.push(Token::Pipe);
            i += 1;
            continue;
        }

        // 普通词
        let mut word = String::new();
        while i < len && !chars[i].is_whitespace() && chars[i] != '|' {
            word.push(chars[i]);
            i += 1;
        }
        if !word.is_empty() {
            tokens.push(Token::Word(word));
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== detect_bash_shortcut ====================

    #[test]
    fn test_detect_bash_shortcut_simple() {
        let result = detect_bash_shortcut("!!@Bash: ls -la");
        assert_eq!(result, Some("ls -la".to_string()));
    }

    #[test]
    fn test_detect_bash_shortcut_with_leading_whitespace() {
        let result = detect_bash_shortcut("  !!@Bash: echo hello");
        assert_eq!(result, Some("echo hello".to_string()));
    }

    #[test]
    fn test_detect_bash_shortcut_not_bash() {
        let result = detect_bash_shortcut("ls -la");
        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_bash_shortcut_single_at() {
        let result = detect_bash_shortcut("!@Bash: ls");
        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_bash_shortcut_stops_at_terminator() {
        let result = detect_bash_shortcut("!!@Bash: ls -la @/Async(test)");
        assert_eq!(result, Some("ls -la".to_string()));
    }

    #[test]
    fn test_detect_bash_shortcut_stops_at_at_slash() {
        let result = detect_bash_shortcut("!!@Bash: ls -la @/");
        assert_eq!(result, Some("ls -la".to_string()));
    }

    #[test]
    fn test_detect_bash_shortcut_empty_args() {
        let result = detect_bash_shortcut("!!@Bash:");
        assert_eq!(result, Some(String::new()));
    }

    // ==================== detect_async_marker ====================

    #[test]
    fn test_detect_async_marker_simple() {
        let result = detect_async_marker("ls -la @/Async(test)");
        assert_eq!(result, Some("test".to_string()));
    }

    #[test]
    fn test_detect_async_marker_with_command() {
        let result = detect_async_marker("!@Write:./test.py @/Async(my_async)");
        assert_eq!(result, Some("my_async".to_string()));
    }

    #[test]
    fn test_detect_async_marker_none() {
        let result = detect_async_marker("ls -la");
        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_async_marker_regular_terminator() {
        let result = detect_async_marker("!@Write:./test.py @/");
        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_async_marker_at_end_only() {
        // @/Async should only match at end of line
        let result = detect_async_marker("ls -la @/Async(test) extra stuff");
        assert_eq!(result, None);
    }

    // ==================== tokenize ====================

    #[test]
    fn test_tokenize_simple_shell() {
        let tokens = tokenize("ls -la").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("ls".to_string()),
            Token::Word("-la".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_nacommand_normal() {
        let tokens = tokenize("!@Write:./test.py").unwrap();
        assert_eq!(tokens, vec![
            Token::NormalPrefix,
            Token::Word("Write".to_string()),
            Token::Word("./test.py".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_nacommand_system_with_mode() {
        let tokens = tokenize("!!@Shell:Watch -i \"abc\" -c 3").unwrap();
        assert_eq!(tokens, vec![
            Token::SystemPrefix,
            Token::Word("Shell".to_string()),
            Token::Word("Watch".to_string()),
            Token::Word("-i".to_string()),
            Token::Word("abc".to_string()),
            Token::Word("-c".to_string()),
            Token::Word("3".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_with_pipe() {
        let tokens = tokenize("ls -la | grep foo").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("ls".to_string()),
            Token::Word("-la".to_string()),
            Token::Pipe,
            Token::Word("grep".to_string()),
            Token::Word("foo".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_with_terminator() {
        let tokens = tokenize("!@Write:./test.py @/").unwrap();
        assert_eq!(tokens, vec![
            Token::NormalPrefix,
            Token::Word("Write".to_string()),
            Token::Word("./test.py".to_string()),
            Token::Terminator,
        ]);
    }

    #[test]
    fn test_tokenize_with_async_marker() {
        let tokens = tokenize("ls -la @/Async(test)").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("ls".to_string()),
            Token::Word("-la".to_string()),
            Token::AsyncMarker("test".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_quoted_string() {
        let tokens = tokenize("echo \"hello world\"").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("hello world".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_pipe_inside_quotes() {
        // Pipe inside double quotes should not be split
        let tokens = tokenize("echo \"hello | world\"").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("hello | world".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_at_slash_inside_quotes() {
        // @/ inside quotes should not be a terminator
        let tokens = tokenize("echo \"foo @/ bar\"").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("foo @/ bar".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_single_quoted_string() {
        let tokens = tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("hello world".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_empty_input() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens, vec![]);
    }

    #[test]
    fn test_tokenize_unclosed_quote() {
        let result = tokenize("echo \"hello world");
        assert!(result.is_err());
    }
}
