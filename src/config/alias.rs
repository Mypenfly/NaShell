use std::collections::HashMap;

/// 展开输入命令中的别名。
///
/// 检查输入的首个词是否匹配 alias 表中的 key，若匹配则替换为对应的 value。
/// 对于 `!cmd` 格式的交互命令，去除 `!` 前缀后匹配。
///
/// # 参数
/// - `input`: 用户输入的原始命令字符串
/// - `aliases`: alias 名称到展开内容的映射
///
/// # 返回
/// 展开后的命令字符串（若无匹配则返回原字符串）
///
/// # 示例
/// ```
/// let aliases = [("ll".to_string(), "ls -la".to_string())].into();
/// assert_eq!(expand_alias("ll", &aliases), "ls -la");
/// assert_eq!(expand_alias("ll -h", &aliases), "ls -la -h");
/// ```
pub fn expand_alias(input: &str, aliases: &HashMap<String, String>) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return input.to_string();
    }

    // 提取首词（可能带 ! 前缀）
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    // 去除 ! 前缀得到实际的命令名
    let cmd_name = first_word.strip_prefix('!').unwrap_or(first_word);

    match aliases.get(cmd_name) {
        Some(alias_value) => {
            // 将首词替换为 alias 展开值，保留其余部分
            let rest = trimmed[first_word.len()..].to_string();
            // 如果原始输入以 ! 开头且 alias 值不含 !，需要保持 ! 前缀
            if first_word.starts_with('!') && !alias_value.starts_with('!') {
                format!("!{}{}", alias_value, rest)
            } else {
                format!("{}{}", alias_value, rest)
            }
        }
        None => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_aliases(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_expand_alias_no_match() {
        let aliases = make_aliases(&[("ll", "ls -la")]);
        let result = expand_alias("gst", &aliases);
        assert_eq!(result, "gst");
    }

    #[test]
    fn test_expand_alias_simple_match() {
        let aliases = make_aliases(&[("ll", "ls -la")]);
        let result = expand_alias("ll", &aliases);
        assert_eq!(result, "ls -la");
    }

    #[test]
    fn test_expand_alias_with_remaining_args() {
        let aliases = make_aliases(&[("gst", "git status")]);
        let result = expand_alias("gst -v", &aliases);
        assert_eq!(result, "git status -v");
    }

    #[test]
    fn test_expand_alias_empty_input() {
        let aliases = make_aliases(&[("ll", "ls -la")]);
        let result = expand_alias("", &aliases);
        assert_eq!(result, "");
    }

    #[test]
    fn test_expand_alias_empty_aliases() {
        let aliases: HashMap<String, String> = HashMap::new();
        let result = expand_alias("ll", &aliases);
        assert_eq!(result, "ll");
    }

    #[test]
    fn test_expand_alias_interactive_command() {
        let aliases = make_aliases(&[("shx", "sudo hx --config ~/helix")]);
        let result = expand_alias("!shx", &aliases);
        assert_eq!(result, "!sudo hx --config ~/helix");
    }

    #[test]
    fn test_expand_alias_interactive_with_args() {
        let aliases = make_aliases(&[("shx", "sudo hx --config ~/helix")]);
        let result = expand_alias("!shx src/main.rs", &aliases);
        assert_eq!(result, "!sudo hx --config ~/helix src/main.rs");
    }

    #[test]
    fn test_expand_alias_match_not_first_word() {
        let aliases = make_aliases(&[("ll", "ls -la")]);
        // Only first word is checked for alias
        let result = expand_alias("echo ll", &aliases);
        assert_eq!(result, "echo ll");
    }
}
