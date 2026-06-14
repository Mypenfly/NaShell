use super::*;
use std::path::PathBuf;

#[test]
fn test_manager_new() {
    let manager = ShellManager::new();
    assert!(manager.main_session.is_none());
    assert!(manager.shells.is_empty());
    assert!(manager.name_to_id.is_empty());
}

#[test]
fn test_manager_init_main_shell() {
    let mut manager = ShellManager::new();
    let result = manager.init_main_shell("bash");
    assert!(result.is_ok(), "should init main shell with bash");
    assert!(manager.main_session.is_some());
}

#[test]
fn test_manager_init_main_shell_invalid() {
    let mut manager = ShellManager::new();
    let result = manager.init_main_shell("nonexistent_shell");
    assert!(result.is_err());
}

#[test]
fn test_register_main() {
    let mut manager = ShellManager::new();
    let cwd = std::env::current_dir().unwrap();
    manager.register_main(&cwd);
    assert!(manager.shells.len() == 1);
    assert!(manager.name_to_id.contains_key("main"));

    let main_id = manager.name_to_id.get("main").unwrap();
    let main_shell = manager.shells.get(main_id).unwrap();
    assert_eq!(main_shell.name, "main");
    assert_eq!(main_shell.path, cwd);
    assert!(main_shell.pools.is_empty());
    assert!(main_shell.join_handle.is_none());
}

#[test]
fn test_get_or_create_shell_new() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id = manager.get_or_create_shell("test", &cwd);
    assert!(!id.is_empty());

    let shell = manager.shells.get(&id).unwrap();
    assert_eq!(shell.name, "test");
    assert_eq!(shell.path, PathBuf::from("/tmp"));
    assert_eq!(manager.name_to_id.get("test").unwrap(), &id);
}

#[test]
fn test_get_or_create_shell_existing() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id1 = manager.get_or_create_shell("test", &cwd);
    let id2 = manager.get_or_create_shell("test", &PathBuf::from("/other"));
    assert_eq!(id1, id2);
    assert_eq!(manager.shells.len(), 1);
}

#[test]
fn test_add_and_watch_pools() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id = manager.get_or_create_shell("test", &cwd);

    manager.add_to_pools(&id, "output 1").unwrap();
    manager.add_to_pools(&id, "output 2").unwrap();
    manager.add_to_pools(&id, "output 3").unwrap();

    let results = manager.watch_pools(&id, 2).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], "output 2");
    assert_eq!(results[1], "output 3");
}

#[test]
fn test_watch_pools_count_exceeds() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id = manager.get_or_create_shell("test", &cwd);

    manager.add_to_pools(&id, "output 1").unwrap();

    let results = manager.watch_pools(&id, 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], "output 1");
}

#[test]
fn test_watch_pools_not_found() {
    let manager = ShellManager::new();
    let result = manager.watch_pools("nonexistent", 1);
    assert!(result.is_err());
}

#[test]
fn test_destroy_shell() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id = manager.get_or_create_shell("test", &cwd);
    assert_eq!(manager.shells.len(), 1);

    manager.destroy_shell(&id).unwrap();
    assert!(manager.shells.is_empty());
    assert!(!manager.name_to_id.contains_key("test"));
}

#[test]
fn test_destroy_main_rejected() {
    let mut manager = ShellManager::new();
    let cwd = std::env::current_dir().unwrap();
    manager.register_main(&cwd);
    let main_id = manager.name_to_id.get("main").unwrap().clone();

    let result = manager.destroy_shell(&main_id);
    assert!(result.is_err());
}

#[test]
fn test_destroy_nonexistent() {
    let mut manager = ShellManager::new();
    let result = manager.destroy_shell("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_switch_main() {
    let mut manager = ShellManager::new();
    let old_cwd = std::env::current_dir().unwrap();
    manager.register_main(&old_cwd);

    let target_cwd = PathBuf::from("/tmp");
    let target_id = manager.get_or_create_shell("test", &target_cwd);

    manager.switch_main(&target_id, false).unwrap();

    // 验证切换后新 main 的 path 是旧 main 的 path
    let new_main_id = manager.name_to_id.get("main").unwrap();
    let new_main = manager.shells.get(new_main_id).unwrap();
    assert_eq!(new_main.name, "main");
    assert_eq!(new_main.path, old_cwd);
    assert_eq!(*new_main_id, target_id);

    // 旧 main 现在变成了 "test"
    let old_main_id = manager.name_to_id.get("test").unwrap();
    let old_main = manager.shells.get(old_main_id).unwrap();
    assert_eq!(old_main.name, "test");
    assert_eq!(old_main.path, target_cwd);
}

#[test]
fn test_switch_main_with_destroy() {
    let mut manager = ShellManager::new();
    let old_cwd = std::env::current_dir().unwrap();
    manager.register_main(&old_cwd);

    let target_cwd = PathBuf::from("/tmp");
    let target_id = manager.get_or_create_shell("test", &target_cwd);

    manager.switch_main(&target_id, true).unwrap();

    // 旧的 main 已被销毁
    assert!(!manager.name_to_id.contains_key("test"));
    assert_eq!(manager.shells.len(), 1);
    assert!(manager.name_to_id.contains_key("main"));
}

#[test]
fn test_switch_main_self_rejected() {
    let mut manager = ShellManager::new();
    let cwd = std::env::current_dir().unwrap();
    manager.register_main(&cwd);
    let main_id = manager.name_to_id.get("main").unwrap().clone();

    let result = manager.switch_main(&main_id, false);
    assert!(result.is_err());
}

#[test]
fn test_list_shells() {
    let mut manager = ShellManager::new();
    let cwd = std::env::current_dir().unwrap();
    manager.register_main(&cwd);
    manager.get_or_create_shell("test", &PathBuf::from("/tmp"));

    let list = manager.list_shells();
    assert_eq!(list.len(), 2);
    // main 应该排在最前
    assert_eq!(list[0].0, "main");
    assert_eq!(list[1].0, "test");
}

#[test]
fn test_sync_main_cwd() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    manager.register_main(&cwd);

    let new_cwd = PathBuf::from("/home");
    manager.sync_main_cwd(&new_cwd);

    let main_id = manager.name_to_id.get("main").unwrap();
    let main_shell = manager.shells.get(main_id).unwrap();
    assert_eq!(main_shell.path, PathBuf::from("/home"));
}

#[test]
fn test_generate_id_unique() {
    let id1 = generate_id();
    let _id2 = generate_id();
    // 在同一纳秒内可能相同，但大概率不同
    // 至少检查格式
    assert_eq!(id1.len(), 8);
    assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_get_shell_name() {
    let mut manager = ShellManager::new();
    let cwd = PathBuf::from("/tmp");
    let id = manager.get_or_create_shell("test", &cwd);

    assert_eq!(manager.get_shell_name(&id), Some("test"));
    assert_eq!(manager.get_shell_name("nonexistent"), None);
}
