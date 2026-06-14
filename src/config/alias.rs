use std::collections::HashMap;

/// 展开输入命令中的别名。
///
/// 检查输入的首个词是否匹配 alias 表中的 key，若匹配则替换为对应的 value。
///
/// # 参数
/// - `input`: 用户输入的原始命令字符串
/// - `aliases`: alias 名称到展开内容的映射
///
/// # 返回
/// 展开后的命令字符串（若无匹配则返回原字符串）
pub fn expand_alias(input: &str, aliases: &HashMap<String, String>) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return input.to_string();
    }

    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    match aliases.get(first_word) {
        Some(alias_value) => {
            let rest = trimmed[first_word.len()..].to_string();
            format!("{}{}", alias_value, rest)
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
    fn test_expand_alias_empty_aliases() {
        let aliases: HashMap<String, String> = HashMap::new();
        let result = expand_alias("ll", &aliases);
        assert_eq!(result, "ll");
    }

    #[test]
    fn test_expand_alias_match_not_first_word() {
        let aliases = make_aliases(&[("ll", "ls -la")]);
        // Only first word is checked for alias
        let result = expand_alias("echo ll", &aliases);
        assert_eq!(result, "echo ll");
    }
}
