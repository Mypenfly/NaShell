use crate::error::NashellError;
use crate::parser::lexer;

/// 从输入中提取 `@/` 截止符之后或空行之后的 long_argument。
///
/// 优先级：
/// - 规则 A（优先）：查找首个 `@/`（包括 `@/Async(name)` 形式），
///   之前为命令部分，之后的同/下行内容为 long_argument
/// - 规则 B（回退）：若无 `@/`，查找首个空行分割
/// - 规则 C：两项皆无，long_argument 为 None
///
/// # 参数
/// - `input`: 完整用户输入字符串（可能含多行）
///
/// # 返回
/// `Result<(String, Option<String>), NashellError>` — (命令部分, long_argument)
/// 跳过截止符（`@/` 或 `@/Async(name)`）后提取 long_argument。
///
/// 若 `@/` 后紧跟 `\n`（即 `@/` 在行末），跳过该 `\n`。
/// 保留后续内容的原始格式。
fn extract_after_terminator(after: &str, terminator_len: usize) -> String {
    let remaining = after[terminator_len..].to_string();
    // 若截止符后紧跟换行（即截止符在行末），跳过该换行
    if let Some(stripped) = remaining.strip_prefix('\n') {
        stripped.to_string()
    } else {
        // 截止符在同一行中间时，剔除前导空格（保留后续换行）
        remaining.trim_start().to_string()
    }
}

pub fn extract_long_argument(input: &str) -> Result<(String, Option<String>), NashellError> {
    // 规则 A：查找首个 @/
    if let Some(pos) = input.find("@/") {
        let command_part = input[..pos].trim_end().to_string();
        let after = &input[pos..];

        // 提取首行用于检测 async marker
        let first_line = after.split('\n').next().unwrap_or(after);
        if let Some(_name) = lexer::detect_async_marker(first_line) {
            // @/Async(name) 作为截止符
            let marker_end = first_line
                .find(')')
                .map(|p| p + 1)
                .unwrap_or(first_line.len());
            let long_arg = extract_after_terminator(after, marker_end);
            let long_arg = long_arg.trim_start_matches('\n').to_string();
            return Ok((command_part, Some(long_arg)));
        }

        // 普通 @/ 截止符
        let long_arg = extract_after_terminator(after, 2);
        return Ok((command_part, Some(long_arg)));
    }

    // 规则 B：查找首个空行
    if let Some(pos) = input.find("\n\n") {
        let command_part = input[..pos].trim_end().to_string();
        let long_arg = input[pos + 2..].trim_start().to_string();
        if long_arg.is_empty() {
            return Ok((command_part, None));
        }
        return Ok((command_part, Some(long_arg)));
    }

    // 规则 C：无 @/ 无空行
    Ok((input.trim().to_string(), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_c_single_line_no_at_slash() {
        let result = extract_long_argument("ls -la").unwrap();
        assert_eq!(result.0, "ls -la");
        assert_eq!(result.1, None);
    }

    #[test]
    fn test_rule_c_single_line_no_blank_line() {
        let result = extract_long_argument("!@Write:./test.py").unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, None);
    }

    #[test]
    fn test_rule_a_at_slash_end_of_line() {
        let input = "!@Write:./test.py @/";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, Some(String::new()));
    }

    #[test]
    fn test_rule_a_at_slash_with_multiline_long_arg() {
        let input = "!@Write:./test.py @/\nx = 1\nprint(x)";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, Some("x = 1\nprint(x)".to_string()));
    }

    #[test]
    fn test_rule_a_at_slash_with_content_on_same_line() {
        let input = "!@Write:./test.py @/ trailing content";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, Some("trailing content".to_string()));
    }

    #[test]
    fn test_rule_a_at_slash_async_marker() {
        let input = "ls -la @/Async(test)";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "ls -la");
        assert_eq!(result.1, Some(String::new()));
    }

    #[test]
    fn test_rule_a_at_slash_async_marker_with_content_after() {
        let input = "ls -la @/Async(test)\nhello\nworld";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "ls -la");
        assert_eq!(result.1, Some("hello\nworld".to_string()));
    }

    #[test]
    fn test_rule_b_blank_line_separation() {
        let input = "!@Write:./test.py\n\nx = 1";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, Some("x = 1".to_string()));
    }

    #[test]
    fn test_rule_b_multiple_command_lines() {
        let input = "ls -la\n-h\n\ncontent line 1\ncontent line 2";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "ls -la\n-h");
        assert_eq!(result.1, Some("content line 1\ncontent line 2".to_string()));
    }

    #[test]
    fn test_rule_a_takes_priority_over_b() {
        // @/ is found, so blank line after is part of long_argument
        let input = "ls -la @/\n\nstill content";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "ls -la");
        assert_eq!(result.1, Some("\nstill content".to_string()));
    }

    #[test]
    fn test_rule_c_no_terminator_no_blank_line() {
        let input = "echo hello world";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "echo hello world");
        assert_eq!(result.1, None);
    }

    #[test]
    fn test_rule_a_at_slash_not_at_line_end() {
        let input = "!@Write:./test.py @/config content";
        let result = extract_long_argument(input).unwrap();
        assert_eq!(result.0, "!@Write:./test.py");
        assert_eq!(result.1, Some("config content".to_string()));
    }
}
