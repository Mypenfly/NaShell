use std::sync::{Arc, Mutex};

use crate::error::NashellError;
use crate::nacommand::cmd::NaCommand;
use crate::shell::manager::ShellManager;

/// 执行 Shell 管理命令。
///
/// 支持四种模式：
/// - 默认（无 mode）：列出所有 Shell 状态表格
/// - Watch：查看指定 shell 的 pools，支持 -i/--id（必传）和 -c/--count（可选，默认 1）
/// - Destroy：销毁指定 shell，支持 -i/--id（必传）
/// - Switch：切换 main shell，支持 -i/--id（必传）和 -d/--destroy（可选）
/// - Help：返回帮助信息
///
/// # 参数
/// - `cmd`: NaCommand 数据结构（cmd="shell"）
/// - `manager`: Shell 管理器（Arc<Mutex> 共享引用）
///
/// # 返回
/// - `Ok(String)`: 命令执行结果
/// - `Err(NashellError)`: 执行错误
pub fn execute_shell_cmd(
    cmd: &NaCommand,
    manager: &Arc<Mutex<ShellManager>>,
) -> Result<String, NashellError> {
    let mode = cmd.mode.as_deref();

    match mode {
        Some("help") => Ok(build_help_text()),
        Some("watch") => execute_watch(cmd, manager),
        Some("destroy") => execute_destroy(cmd, manager),
        Some("switch") => execute_switch(cmd, manager),
        // 默认模式：列出所有 Shell 状态
        _ => execute_list(manager),
    }
}

/// 提取命令行选项。
///
/// 支持短选项（-x value）和长选项（--xxx value）。
/// -i/--id: 提取 id 值
/// -c/--count: 提取 count 值（解析为 usize）
/// -d/--destroy: 检查是否存在
pub(crate) fn parse_options(args: &[String]) -> (Option<String>, Option<usize>, bool) {
    let mut id: Option<String> = None;
    let mut count: Option<usize> = None;
    let mut destroy = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--id" => {
                if i + 1 < args.len() {
                    id = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "-c" | "--count" => {
                if i + 1 < args.len() {
                    count = args[i + 1].parse::<usize>().ok();
                    i += 1;
                }
            }
            "-d" | "--destroy" => {
                destroy = true;
            }
            _ => {}
        }
        i += 1;
    }

    (id, count, destroy)
}

/// 默认模式：列出所有 Shell 状态表格。
fn execute_list(manager: &Arc<Mutex<ShellManager>>) -> Result<String, NashellError> {
    let mgr = manager.lock().map_err(|e| NashellError::Execute {
        command: "shell".to_string(),
        exit_code: None,
        stderr: format!("获取 ShellManager 锁失败: {}", e),
    })?;

    let shells = mgr.list_shells();

    if shells.is_empty() {
        return Ok("Shells states\n  (暂无 shell)".to_string());
    }

    let mut output = String::from("Shells states\n");
    // 计算列宽
    let max_name = shells.iter().map(|s| s.0.len()).max().unwrap_or(4).max(4);
    let max_id = shells.iter().map(|s| s.1.len()).max().unwrap_or(2).max(2);
    let max_path = shells
        .iter()
        .map(|s| s.2.len())
        .max()
        .unwrap_or(4)
        .max(4);

    output.push_str(&format!(
        "  {:<width_name$}  {:<width_id$}  {:<width_path$}  pools_count\n",
        "name",
        "id",
        "path",
        width_name = max_name,
        width_id = max_id,
        width_path = max_path,
    ));

    for (name, id, path, pools_count) in shells {
        output.push_str(&format!(
            "  {:<width_name$}  {:<width_id$}  {:<width_path$}  {}\n",
            name,
            id,
            path,
            pools_count,
            width_name = max_name,
            width_id = max_id,
            width_path = max_path,
        ));
    }

    // 移除末尾换行
    output.pop();

    Ok(output)
}

/// Watch 模式：查看指定 shell 的 pools。
fn execute_watch(
    cmd: &NaCommand,
    manager: &Arc<Mutex<ShellManager>>,
) -> Result<String, NashellError> {
    let (id, count, _) = parse_options(&cmd.args);

    let shell_id = id.ok_or_else(|| NashellError::Execute {
        command: "shell watch".to_string(),
        exit_code: None,
        stderr: "缺少必传参数: -i/--id".to_string(),
    })?;

    let actual_count = count.unwrap_or(1);

    let mgr = manager.lock().map_err(|e| NashellError::Execute {
        command: "shell".to_string(),
        exit_code: None,
        stderr: format!("获取 ShellManager 锁失败: {}", e),
    })?;

    let shell_name = mgr
        .get_shell_name(&shell_id)
        .unwrap_or("unknown")
        .to_string();

    let pools = mgr.watch_pools(&shell_id, actual_count)?;

    let mut output = format!(
        "shell pools\n  name: {}    id: {}\n",
        shell_name, shell_id
    );

    if pools.is_empty() {
        output.push_str("  (pools 为空)");
    } else {
        let total = mgr
            .shells
            .get(&shell_id)
            .map(|s| s.pools.len())
            .unwrap_or(0);

        let start_index = total - pools.len();
        for (i, content) in pools.iter().enumerate() {
            output.push_str(&format!(
                "  pool index ({}):\n    {}\n",
                start_index + i,
                content
            ));
        }
    }

    // 移除末尾换行
    output.pop();

    Ok(output)
}

/// Destroy 模式：销毁指定 shell。
fn execute_destroy(
    cmd: &NaCommand,
    manager: &Arc<Mutex<ShellManager>>,
) -> Result<String, NashellError> {
    let (id, _, _) = parse_options(&cmd.args);

    let shell_id = id.ok_or_else(|| NashellError::Execute {
        command: "shell destroy".to_string(),
        exit_code: None,
        stderr: "缺少必传参数: -i/--id".to_string(),
    })?;

    let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
        command: "shell".to_string(),
        exit_code: None,
        stderr: format!("获取 ShellManager 锁失败: {}", e),
    })?;

    let shell_name = mgr
        .get_shell_name(&shell_id)
        .unwrap_or("unknown")
        .to_string();

    mgr.destroy_shell(&shell_id)?;

    Ok(format!(
        "shell has been destroyed\n  name: {}    id: {}",
        shell_name, shell_id
    ))
}

/// Switch 模式：切换 main shell。
fn execute_switch(
    cmd: &NaCommand,
    manager: &Arc<Mutex<ShellManager>>,
) -> Result<String, NashellError> {
    let (id, _, destroy_old) = parse_options(&cmd.args);

    let shell_id = id.ok_or_else(|| NashellError::Execute {
        command: "shell switch".to_string(),
        exit_code: None,
        stderr: "缺少必传参数: -i/--id".to_string(),
    })?;

    let mut mgr = manager.lock().map_err(|e| NashellError::Execute {
        command: "shell".to_string(),
        exit_code: None,
        stderr: format!("获取 ShellManager 锁失败: {}", e),
    })?;

    let old_main_name = {
        let main_id = mgr.name_to_id.get("main").cloned().unwrap_or_default();
        mgr.shells
            .get(&main_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "main".to_string())
    };
    let old_main_id = mgr.name_to_id.get("main").cloned().unwrap_or_default();
    let target_name = mgr
        .get_shell_name(&shell_id)
        .unwrap_or("unknown")
        .to_string();

    mgr.switch_main(&shell_id, destroy_old)?;

    let mut output = format!(
        "main shell has been switched from {}({}) to {}({})",
        old_main_name, old_main_id, target_name, shell_id
    );

    if destroy_old {
        output.push_str(&format!(
            "\nold shell destroyed\n  name: {}    id: {}",
            old_main_name, old_main_id
        ));
    }

    Ok(output)
}

/// 构建 Shell 命令的帮助文本（带 ANSI 颜色）。
fn build_help_text() -> String {
    let c = |s: &str| format!("\x1b[96m\x1b[1m{}\x1b[0m", s);
    let h = |s: &str| format!("\x1b[94m{}\x1b[0m", s);
    let g = |s: &str| format!("\x1b[32m{}\x1b[0m", s);
    let y = |s: &str| format!("\x1b[93m{}\x1b[0m", s);

    format!(
        "{}\n  \
         管理 NaShell 内部的 Shell 线程。\n\n  \
         {}\n    Shell:       列出所有 Shell 状态表格\n    \
         Shell:Watch   查看指定 shell 的 pools（-i id [-c count]）\n    \
         Shell:Destroy 销毁指定 shell（-i id）\n    \
         Shell:Switch  切换 main shell（-i id [-d]）\n\n  \
         {}\n    {}\n    {}\n    {}\n    {}\n\n  \
         {}\n  Shell 命令使用 id 而非 name 来定位目标。\n  \
         id 在 shell 创建时自动分配，可通过默认模式查看。",
        c("Shell"),
        h("模式:"),
        h("使用示例:"),
        g("!!@Shell:"),
        g("!!@Shell:Watch -i abc123 -c 3"),
        g("!!@Shell:Destroy -i abc123"),
        g("!!@Shell:Switch -i abc123 -d"),
        y("注意:"),
    )
}

#[cfg(test)]
#[path = "shell_cmd_tests.rs"]
mod tests;
