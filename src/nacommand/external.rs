use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::app::CmdMeta;
use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;

/// 确保 `/tmp/nashell/` 目录存在。
fn ensure_tmp_nashell_dir() -> Result<PathBuf, NashellError> {
    let dir = PathBuf::from("/tmp/nashell");
    std::fs::create_dir_all(&dir).map_err(|e| NashellError::Io {
        path: Some(dir.to_string_lossy().to_string()),
        source: e,
    })?;
    Ok(dir)
}

/// 生成随机临时文件路径。
fn random_temp_path(dir: &Path, ext: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let file_name = format!("nashell_{}_{}{}", std::process::id(), nanos, ext);
    dir.join(file_name)
}

/// 解析单个路径，支持相对于配置文件目录的相对路径。
///
/// 仅对显式相对路径（以 `./` 或 `../` 开头）做 config_dir 解析。
/// 绝对路径和裸命令名（如 `python3`、`echo`）原样返回。
fn resolve_single_path(path_str: &str, config_dir: Option<&Path>) -> String {
    let path = Path::new(path_str);
    if path.is_absolute() {
        return path_str.to_string();
    }
    // 仅对显式相对路径（./xxx 或 ../xxx）做解析，裸命令名不改动
    if path_str.starts_with("./") || path_str.starts_with("../") {
        if let Some(dir) = config_dir {
            let resolved = dir.join(path_str);
            return resolved.to_string_lossy().to_string();
        }
    }
    path_str.to_string()
}

/// 将 exec 字符串拆分为程序名和初始参数，并对其中相对路径做解析。
///
/// `exec` 可包含空格，如 `"python3 ./web_search.py"`。
/// 首个 token 作为程序名，后续 token 作为初始参数。
/// 两者中的相对路径均相对于 `config_dir` 解析。
///
/// # 返回
/// `(已解析的程序路径, 已解析的初始参数列表)`
fn resolve_exec_parts(exec: &str, config_dir: Option<&Path>) -> (String, Vec<String>) {
    let tokens: Vec<String> = exec.split_whitespace().map(|s| s.to_string()).collect();
    if tokens.is_empty() {
        return (String::new(), vec![]);
    }

    let program = resolve_single_path(&tokens[0], config_dir);
    let pre_args: Vec<String> = tokens[1..]
        .iter()
        .map(|t| resolve_single_path(t, config_dir))
        .collect();

    (program, pre_args)
}

/// 执行用户配置的外部 NaCommand。
///
/// 根据 `CmdMeta` 的配置执行外部程序：
/// - Help 模式：对 exec 程序传入 `--help`，透传输出
/// - 正常模式：构建命令行参数，启动进程并捕获 stdout/stderr
///   - 若无 `exec_script`：long_argument 作为字符串传给 exec 的最后一个参数
///   - 若有 `exec_script`：long_argument 保存为临时文件，临时文件路径作为 exec 的参数
///
/// `exec` 字段可包含空格（如 `"python3 ./script.py"`），
/// 首个 token 为程序名，其余为初始参数。
///
/// # 参数
/// - `cmd_meta`: 命令元数据（包含 exec 路径、long_argument 支持等）
/// - `nacommand`: NaCommand 数据结构
/// - `config_dir`: 配置文件所在目录（用于解析 exec 相对路径）
///
/// # 返回
/// - `Ok(String)`: 命令执行结果（stdout + stderr，保留 ANSI 码）
/// - `Err(NashellError)`: 执行错误
pub fn execute_external(
    cmd_meta: &CmdMeta,
    nacommand: &NaCommand,
    config_dir: Option<&Path>,
) -> Result<String, NashellError> {
    let (exec_program, pre_args) = resolve_exec_parts(&cmd_meta.exec, config_dir);
    let mut cmd_args: Vec<String> = pre_args.clone();

    // Help 模式：只传 --help
    if nacommand.mode.as_deref().map_or(false, |m| m == "help") {
        cmd_args.push("--help".to_string());
    } else {
        // 正常模式：
        // 1. mode 作为参数（如果有）
        if let Some(ref mode) = nacommand.mode {
            cmd_args.push(mode.clone());
        }

        // 2. 普通 args
        for arg in &nacommand.args {
            cmd_args.push(arg.clone());
        }

        // 3. long_argument 处理
        if let Some(ref long_arg) = nacommand.long_argument {
            if !long_arg.is_empty() {
                if let Some(ref ext) = cmd_meta.exec_script {
                    // exec_script 模式：保存为临时文件
                    let tmp_dir = ensure_tmp_nashell_dir()?;
                    let tmp_path = random_temp_path(&tmp_dir, ext);
                    std::fs::write(&tmp_path, long_arg).map_err(|e| NashellError::Io {
                        path: Some(tmp_path.to_string_lossy().to_string()),
                        source: e,
                    })?;
                    cmd_args.push(tmp_path.to_string_lossy().to_string());
                } else if cmd_meta.long_argument {
                    // 无 exec_script，直接作为最后一个参数
                    cmd_args.push(long_arg.clone());
                }
            }
        }
    }

    // 启动进程并捕获输出
    let child = Command::new(&exec_program)
        .args(&cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| NashellError::Io {
            path: Some(exec_program.clone()),
            source: e,
        })?;

    let output = child.wait_with_output().map_err(|e| NashellError::Io {
        path: Some(exec_program),
        source: e,
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // 清理临时文件（仅 exec_script 模式且 long_argument 不为空时）
    if nacommand.mode.as_deref().map_or(true, |m| m != "help")
        && cmd_meta.exec_script.is_some()
        && nacommand.long_argument.as_ref().map_or(false, |s| !s.is_empty())
    {
        if let Some(last_arg) = cmd_args.last() {
            let tmp_path = Path::new(last_arg);
            if tmp_path.starts_with("/tmp/nashell/") {
                if let Err(e) = std::fs::remove_file(tmp_path) {
                    log::warn!("清理临时文件失败 '{}': {}", last_arg, e);
                }
            }
        }
    }

    let mut result = stdout;
    if !stderr.is_empty() {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&stderr);
    }

    Ok(result.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CmdMeta, Level};
    use crate::nacommand::cmd::{NaCommand, NaLevel};

    fn make_cmd_meta(exec: &str, long_argument: bool, exec_script: Option<&str>) -> CmdMeta {
        CmdMeta {
            level: Level::Normal,
            name: "testcmd".to_string(),
            exec: exec.to_string(),
            long_argument,
            exec_script: exec_script.map(|s| s.to_string()),
            known_modes: vec![],
        }
    }

    fn make_nacommand(args: Vec<&str>, long_argument: Option<&str>, mode: Option<&str>) -> NaCommand {
        NaCommand {
            level: NaLevel::Normal,
            cmd: "testcmd".to_string(),
            mode: mode.map(|s| s.to_string()),
            args: args.iter().map(|s| s.to_string()).collect(),
            long_argument: long_argument.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_execute_external_simple_echo() {
        let meta = make_cmd_meta("echo", false, None);
        let cmd = make_nacommand(vec!["hello"], None, None);

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_execute_external_with_long_argument_last() {
        let meta = make_cmd_meta("echo", true, None);
        let cmd = make_nacommand(vec!["-n"], Some("long_content"), None);

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("long_content"));
    }

    #[test]
    fn test_execute_external_help_mode() {
        let meta = make_cmd_meta("echo", false, None);
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "testcmd".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("--help"));
    }

    #[test]
    fn test_execute_external_with_mode() {
        let meta = make_cmd_meta("echo", false, None);
        let cmd = make_nacommand(vec!["arg1"], None, Some("today"));

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("today"));
        assert!(result.contains("arg1"));
    }

    #[test]
    fn test_execute_external_exec_script_temp_file() {
        let script_content = "hello from script";

        // 动态查找 cat 路径（兼容 NixOS 等非标准路径环境）
        let cat_path = std::process::Command::new("which")
            .arg("cat")
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "cat".to_string());

        let meta = make_cmd_meta(&cat_path, true, Some(".txt"));
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "testcmd".to_string(),
            mode: None,
            args: vec![],
            long_argument: Some(script_content.to_string()),
        };

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert_eq!(result, script_content);
    }

    #[test]
    fn test_execute_external_exec_not_found() {
        let meta = make_cmd_meta("nonexistent_program_xyz", false, None);
        let cmd = make_nacommand(vec![], None, None);

        let result = execute_external(&meta, &cmd, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_external_with_args_and_mode() {
        let meta = make_cmd_meta("echo", false, None);
        let cmd = make_nacommand(vec!["--verbose", "extra"], None, Some("search"));

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("search"));
        assert!(result.contains("--verbose"));
        assert!(result.contains("extra"));
    }

    #[test]
    fn test_execute_external_no_long_argument_configured() {
        let meta = make_cmd_meta("echo", false, None);
        let cmd = make_nacommand(vec!["arg1", "arg2"], None, None);

        let result = execute_external(&meta, &cmd, None).unwrap();
        assert!(result.contains("arg1"));
        assert!(result.contains("arg2"));
    }

    #[test]
    fn test_execute_external_exec_with_spaces() {
        // exec 包含空格：程序名 + 脚本路径
        let meta = make_cmd_meta("echo hello_from_prearg", false, None);
        let cmd = make_nacommand(vec!["user_arg"], None, None);

        let result = execute_external(&meta, &cmd, None).unwrap();
        // echo 会输出: hello_from_prearg user_arg
        assert!(result.contains("hello_from_prearg"));
        assert!(result.contains("user_arg"));
    }

    #[test]
    fn test_execute_external_help_with_pre_args() {
        // exec 包含空格，help 模式时 pre_args 仍保留
        let meta = make_cmd_meta("echo script.py", false, None);
        let cmd = NaCommand {
            level: NaLevel::Normal,
            cmd: "testcmd".to_string(),
            mode: Some("help".to_string()),
            args: vec![],
            long_argument: None,
        };

        let result = execute_external(&meta, &cmd, None).unwrap();
        // 输出应为: script.py --help
        assert!(result.contains("script.py"));
        assert!(result.contains("--help"));
    }
}
