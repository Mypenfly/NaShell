pub mod lexer;
pub mod long_arg;
pub mod pipeline;
pub mod syntax;

use crate::error::NashellError;
use syntax::{CmdType, RawCmd, RawCommands};

/// 将单个命令段（已按管道分割）解析为 `RawCmd`。
///
/// 通过 lexer tokenize 识别前缀，确定 `CmdType`，并提取命令名和参数。
fn parse_cmd_segment(segment: &str) -> Result<RawCmd, NashellError> {
    let tokens = lexer::tokenize(segment)?;

    if tokens.is_empty() {
        return Err(NashellError::Parse {
            context: segment.to_string(),
            detail: "空的命令段".to_string(),
        });
    }

    let (cmd_type, word_start) = match &tokens[0] {
        lexer::Token::SystemPrefix => (CmdType::NaCommandSystem, 1),
        lexer::Token::NormalPrefix => (CmdType::NaCommandNormal, 1),
        lexer::Token::Word(_) => (CmdType::Shell, 0),
        lexer::Token::Pipe | lexer::Token::Terminator | lexer::Token::AsyncMarker(_) => {
            return Err(NashellError::Parse {
                context: segment.to_string(),
                detail: format!("意外的 token: {:?}", tokens[0]),
            });
        }
    };

    let words: Vec<&str> = tokens[word_start..]
        .iter()
        .filter_map(|t| match t {
            lexer::Token::Word(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();

    let cmd = words.first().map(|s| s.to_string()).unwrap_or_default();
    let args = words.iter().skip(1).map(|s| s.to_string()).collect();

    Ok(RawCmd {
        cmd_type,
        cmd,
        args,
    })
}

/// 解析用户输入为 `RawCommands` 结构体。
///
/// 整合完整的解析流程：
/// 1. 检测 `!!@Bash:` 快捷方式
/// 2. 检测 `@/Async(name)` 异步标记
/// 3. 提取 long_argument
/// 4. 按管道分割命令部分
/// 5. 逐个命令段解析 `CmdType` / `cmd` / `args`
///
/// # 参数
/// - `input`: 用户输入的完整字符串（可能含多行）
///
/// # 返回
/// `Result<RawCommands, NashellError>` — 解析后的命令集合
pub fn parse(input: &str) -> Result<RawCommands, NashellError> {
    // 阶段 1：检测 !!@Bash: 快捷方式
    if let Some(bash_args) = lexer::detect_bash_shortcut(input) {
        let first_line = input.split('\n').next().unwrap_or("");
        let async_name = lexer::detect_async_marker(first_line);
        return Ok(RawCommands {
            commands: vec![RawCmd {
                cmd_type: CmdType::NaCommandSystem,
                cmd: "bash".to_string(),
                args: vec![bash_args],
            }],
            long_argument: None,
            pre_out: None,
            async_name,
        });
    }

    // 阶段 2：检测 @/Async(name)
    let async_name = lexer::detect_async_marker(
        input.split('\n').next().unwrap_or(""),
    );

    // 阶段 3：提取 long_argument
    let (command_part, long_argument) = long_arg::extract_long_argument(input)?;

    // 阶段 4：管道分割
    let segments = pipeline::split_pipeline(&command_part)?;

    // 阶段 5：逐个解析命令段
    let commands: Vec<RawCmd> = segments
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_cmd_segment(s))
        .collect::<Result<Vec<_>, _>>()?;

    // 将空字符串的 long_argument 归一化为 None
    let long_argument = long_argument.filter(|s| !s.is_empty());

    Ok(RawCommands {
        commands,
        long_argument,
        pre_out: None,
        async_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_shell() {
        let result = parse("ls -la").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::Shell);
        assert_eq!(result.commands[0].cmd, "ls");
        assert_eq!(result.commands[0].args, vec!["-la"]);
        assert_eq!(result.long_argument, None);
        assert_eq!(result.async_name, None);
    }

    #[test]
    fn test_parse_nacommand_normal() {
        let result = parse("!@Open:./src -l 200").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::NaCommandNormal);
        assert_eq!(result.commands[0].cmd, "Open");
        assert_eq!(result.commands[0].args, vec!["./src", "-l", "200"]);
    }

    #[test]
    fn test_parse_nacommand_system_with_mode() {
        let result = parse("!!@Shell:Watch -i \"abc\" -c 3").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::NaCommandSystem);
        assert_eq!(result.commands[0].cmd, "Shell");
        assert_eq!(result.commands[0].args, vec!["Watch", "-i", "abc", "-c", "3"]);
    }

    #[test]
    fn test_parse_pipe_split() {
        let result = parse("ls | grep foo").unwrap();
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands[0].cmd_type, CmdType::Shell);
        assert_eq!(result.commands[0].cmd, "ls");
        assert_eq!(result.commands[1].cmd_type, CmdType::Shell);
        assert_eq!(result.commands[1].cmd, "grep");
        assert_eq!(result.commands[1].args, vec!["foo"]);
    }

    #[test]
    fn test_parse_pipe_with_nacommand() {
        let result = parse("ls | !@Write:./out.txt @/\nhello").unwrap();
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.commands[0].cmd_type, CmdType::Shell);
        assert_eq!(result.commands[1].cmd_type, CmdType::NaCommandNormal);
        assert_eq!(result.commands[1].cmd, "Write");
        assert_eq!(result.long_argument, Some("hello".to_string()));
    }

    #[test]
    fn test_parse_bash_shortcut() {
        let result = parse("!!@Bash: ls -la").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::NaCommandSystem);
        assert_eq!(result.commands[0].cmd, "bash");
        assert_eq!(result.commands[0].args, vec!["ls -la"]);
    }

    #[test]
    fn test_parse_bash_shortcut_with_async() {
        let result = parse("!!@Bash: ls -la @/Async(back)").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::NaCommandSystem);
        assert_eq!(result.commands[0].cmd, "bash");
        assert_eq!(result.commands[0].args, vec!["ls -la"]);
        assert_eq!(result.async_name, Some("back".to_string()));
    }

    #[test]
    fn test_parse_with_long_argument_multi_line() {
        let input = "!@Write:./test.py @/\nx = 1\nprint(x)";
        let result = parse(input).unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::NaCommandNormal);
        assert_eq!(result.commands[0].cmd, "Write");
        assert_eq!(result.commands[0].args, vec!["./test.py"]);
        assert_eq!(result.long_argument, Some("x = 1\nprint(x)".to_string()));
    }

    #[test]
    fn test_parse_with_async_marker() {
        let result = parse("ls -la @/Async(test)").unwrap();
        assert_eq!(result.commands.len(), 1);
        assert_eq!(result.commands[0].cmd_type, CmdType::Shell);
        assert_eq!(result.commands[0].cmd, "ls");
        assert_eq!(result.async_name, Some("test".to_string()));
    }

    #[test]
    fn test_parse_empty_input() {
        let result = parse("").unwrap();
        assert!(result.commands.is_empty());
        assert_eq!(result.long_argument, None);
    }
}
