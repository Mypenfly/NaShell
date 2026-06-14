use super::*;
use crate::nacommand::cmd::{NaCommand, NaLevel};
use std::path::PathBuf;
use std::sync::Arc;

fn test_manager() -> Arc<Mutex<ShellManager>> {
    let mut mgr = ShellManager::new();
    let cwd = std::env::current_dir().unwrap();
    mgr.register_main(&cwd);
    let tmp = PathBuf::from("/tmp");
    mgr.get_or_create_shell("test", &tmp);
    Arc::new(Mutex::new(mgr))
}

#[test]
fn test_execute_shell_list() {
    let mgr = test_manager();
    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: None,
        args: vec![],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("Shells states"));
    assert!(result.contains("main"));
    assert!(result.contains("test"));
    assert!(result.contains("pools_count"));
}

#[test]
fn test_execute_shell_list_empty() {
    let mgr = Arc::new(Mutex::new(ShellManager::new()));
    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: None,
        args: vec![],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("Shells states"));
    assert!(result.contains("暂无 shell"));
}

#[test]
fn test_execute_shell_watch_empty_pools() {
    let mgr = test_manager();
    let main_id = {
        let m = mgr.lock().unwrap();
        m.name_to_id.get("main").cloned().unwrap()
    };

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("watch".to_string()),
        args: vec!["-i".to_string(), main_id.clone(), "-c".to_string(), "5".to_string()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("shell pools"));
    assert!(result.contains("pools 为空"));
}

#[test]
fn test_execute_shell_watch_with_pools() {
    let mgr = test_manager();
    let main_id;
    {
        let mut m = mgr.lock().unwrap();
        main_id = m.name_to_id.get("main").cloned().unwrap();
        m.add_to_pools(&main_id, "output A").unwrap();
        m.add_to_pools(&main_id, "output B").unwrap();
    }

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("watch".to_string()),
        args: vec!["-i".to_string(), main_id.clone(), "-c".to_string(), "1".to_string()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("shell pools"));
    assert!(result.contains("output B"));
    assert!(!result.contains("output A"));
}

#[test]
fn test_execute_shell_watch_missing_id() {
    let mgr = test_manager();
    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("watch".to_string()),
        args: vec!["-c".to_string(), "3".to_string()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr);
    assert!(result.is_err());
}

#[test]
fn test_execute_shell_destroy() {
    let mgr = Arc::new(Mutex::new({
        let mut mgr = ShellManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.register_main(&cwd);
        let tmp = PathBuf::from("/tmp");
        mgr.get_or_create_shell("test", &tmp);
        mgr
    }));
    let test_id = {
        let m = mgr.lock().unwrap();
        m.name_to_id.get("test").cloned().unwrap()
    };

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("destroy".to_string()),
        args: vec!["-i".to_string(), test_id.clone()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("shell has been destroyed"));

    // 确认 test 已被移除
    let m = mgr.lock().unwrap();
    assert!(!m.name_to_id.contains_key("test"));
}

#[test]
fn test_execute_shell_destroy_main_rejected() {
    let mgr = Arc::new(Mutex::new({
        let mut mgr = ShellManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.register_main(&cwd);
        mgr
    }));
    let main_id = {
        let m = mgr.lock().unwrap();
        m.name_to_id.get("main").cloned().unwrap()
    };

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("destroy".to_string()),
        args: vec!["-i".to_string(), main_id],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr);
    assert!(result.is_err());
}

#[test]
fn test_execute_shell_switch() {
    let mgr = test_manager();
    let test_id = {
        let m = mgr.lock().unwrap();
        m.name_to_id.get("test").cloned().unwrap()
    };

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("switch".to_string()),
        args: vec!["-i".to_string(), test_id.clone()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("main shell has been switched"));

    // 验证 main 现在指向原 test 的 shell
    let m = mgr.lock().unwrap();
    let new_main_id = m.name_to_id.get("main").unwrap();
    assert_eq!(*new_main_id, test_id);
}

#[test]
fn test_execute_shell_switch_with_destroy() {
    let mgr = test_manager();
    let test_id = {
        let m = mgr.lock().unwrap();
        m.name_to_id.get("test").cloned().unwrap()
    };

    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("switch".to_string()),
        args: vec!["-i".to_string(), test_id.clone(), "-d".to_string()],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("main shell has been switched"));
    assert!(result.contains("old shell destroyed"));

    // 旧 main 已被销毁，只剩一个 shell
    let m = mgr.lock().unwrap();
    assert_eq!(m.shells.len(), 1);
    assert!(m.name_to_id.contains_key("main"));
}

#[test]
fn test_execute_shell_help() {
    let mgr = test_manager();
    let cmd = NaCommand {
        level: NaLevel::System,
        cmd: "shell".to_string(),
        mode: Some("help".to_string()),
        args: vec![],
        long_argument: None,
    };
    let result = execute_shell_cmd(&cmd, &mgr).unwrap();
    assert!(result.contains("Shell"));
    assert!(result.contains("管理 NaShell"));
}

#[test]
fn test_parse_options() {
    let args = vec![
        "-i".to_string(),
        "abc123".to_string(),
        "-c".to_string(),
        "3".to_string(),
        "-d".to_string(),
    ];
    let (id, count, destroy) = parse_options(&args);
    assert_eq!(id, Some("abc123".to_string()));
    assert_eq!(count, Some(3));
    assert!(destroy);
}

#[test]
fn test_parse_options_long_form() {
    let args = vec![
        "--id".to_string(),
        "xyz789".to_string(),
        "--count".to_string(),
        "10".to_string(),
        "--destroy".to_string(),
    ];
    let (id, count, destroy) = parse_options(&args);
    assert_eq!(id, Some("xyz789".to_string()));
    assert_eq!(count, Some(10));
    assert!(destroy);
}

#[test]
fn test_parse_options_defaults() {
    let args: Vec<String> = vec![];
    let (id, count, destroy) = parse_options(&args);
    assert_eq!(id, None);
    assert_eq!(count, None);
    assert!(!destroy);
}
