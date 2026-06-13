# Phase 4 PTY 集成 Bug 报告

## 概述

Phase 4 实现中尝试将 shell 命令执行从 `bash -c` 子进程模式迁移到持久 PTY 会话模式。迁移过程中引入了多个连锁问题。最终采用**方案 B：`script -e -q -c` 一次性 PTY** 替代持久 PTY。

---

## Bug #1: 所有 shell 命令无状态（cd 无效）

**严重程度**: P0 | **状态**: ✅ 已解决

**症状**: 输入 `cd /tmp` 后 `pwd` 仍显示原目录。

**根因**: `exec_captured()` 使用 `Command::output()` 每次创建新进程。`cd` 在新进程内改目录后进程退出，父进程目录未变。

**解决方案**: 在 `dispatch()` 中拦截 `cd` 命令，调用 `exec_cd()` → `std::env::set_current_dir()`，提示符通过 `current_dir()` 自动同步。

**位置**: `src/executor/mod.rs`, `src/executor/shell_exec.rs:exec_cd()`

---

## Bug #2: ANSI 颜色丢失 / eza 无输出

**严重程度**: P0 | **状态**: ✅ 已解决

**症状**: `ls`、`jj` 等命令输出无颜色；`eza` 完全无输出。

**根因**: `Command::output()` 将 stdout 作为 pipe 捕获而非 TTY。程序通过 `isatty()` 检测到非终端环境后抑制颜色或拒绝输出。eza 在非 TTY 下甚至完全不写任何输出（strace 确认 `write()` 从未被调用）。

**解决方案**: 使用 `script -e -q -c "{shell} -c '{command}'" /dev/null` 分配 PTY，命令感知到 TTY 后正常输出颜色。传入 `Stdio::inherit()` 使 `script` 能读取终端尺寸。

**位置**: `src/executor/shell_exec.rs:exec_captured()`

---

## Bug #3: PTY 方式执行导致程序卡死

**严重程度**: P0 | **状态**: ✅ 已解决（弃用持久 PTY）

**症状**: 输入任何命令后程序卡死，无输出、无新提示符。

**根因链**（均已通过弃用持久 PTY 消除）:

### 3a. 后台读取线程提前退出
使用 `mpsc::channel` + 后台线程持续读取 PTY 输出。`portable-pty` 以 `O_NONBLOCK` 打开 PTY master，`read()` 无数据时返回 `WouldBlock`。线程将所有 `Err` 当致命错误退出 → channel 断开 → `send_command` 永久等待。

### 3b. 哨兵检测被命令回显干扰
`{cmd}; echo MARKER` 模式中 bash 会回显整条命令（含哨兵），回显行中提前匹配 break，真正输出未被读取。

### 3c. PS1 初始化竞态
`export PS1='MARKER'` 时命令本身包含哨兵文本，需消费 2 个哨兵。bash 启动时间不确定（bashrc），导致 10s 超时。

### 3d. 非阻塞 I/O 轮询引入延迟
`O_NONBLOCK` + `sleep(10ms)` 累积延迟。（即使修了 O_NONBLOCK 也仍有 PROMPT_COMMAND 干扰问题）

**最终方案**: 放弃持久 PTY，用 `script -e -q -c` 一次性 PTY。无哨兵、无状态机、无需 I/O 轮询。

---

## Bug #4: 启动缓慢

**严重程度**: P1 | **状态**: ✅ 已解决

**症状**: 程序启动后需等待数秒才出现提示符。

**根因**: `spawn_pty_session` 中等待 2 个 sentinel 的循环（最长 10s/15s），加 bash 启动 + bashrc 加载时间。

**解决方案**: 不再创建持久 PTY，启动延迟归零。

---

## Bug #5: nushell 在 `script` 下报 "0 columns"

**严重程度**: P0 | **状态**: ✅ 已解决

**症状**: `nu -c 'ls'` → `Couldn't fit table into 0 columns!`

**根因**: `script` 的 stdout 被 Rust 通过 pipe 捕获时，`script` 无法从 stdout 查询终端尺寸，创建的 PTY 尺寸为 0×0。nushell 依赖终端宽度渲染表格。

**解决方案**: 使用 `Command::spawn()` + `Stdio::inherit()` 将真实终端 stdin 传给 `script`，`script` 从 stdin 获取终端尺寸并正确设置 PTY。

**位置**: `src/executor/shell_exec.rs`

---

## Bug #6: 空输入回车直接退出

**严重程度**: P1 | **状态**: ✅ 已解决

**症状**: 命令行为空时按回车直接退出程序。

**根因**: `read_multiline()` 将空首行返回 `Ok(None)`，REPL 循环将其解释为退出信号。

**解决方案**: `read_multiline` 中空首行改为返回 `Ok(Some(String::new()))`，由调用方的 `if input.is_empty() { continue; }` 处理。

**位置**: `src/repl/input.rs`

---

## 启发与经验

### 1. "复杂性"是最大的 bug

持久 PTY 方案引入了至少 5 个相互纠缠的问题（I/O 模型、哨兵检测、初始化竞态、PROMPT_COMMAND 干扰、O_NONBLOCK 语义），每个修复又引入新问题。`script -e -q -c` 只有一行代码，零状态。

### 2. 优先使用 Unix 工具链

`script` 是成熟工具，已解决 PTY 分配、TTY 尺寸、退出码传递等问题。自建持久 PTY 需要重新解决所有这些问题。

### 3. `Command::output()` vs `Command::spawn()` 的 trade-off

`output()` 方便但会断开 stdin。当子进程需要从 stdin 获取 TTY 信息时（如 `script` 查询终端尺寸），必须用 `spawn()` + `Stdio::inherit()`。

### 4. `-c` 模式与状态保持的矛盾

`bash -c` / `nu -c` 每次创建新进程，无法保持 shell 状态（`cd`、环境变量）。对必须保持状态的操作，在 Rust 侧拦截是更干净的做法。

### 5. 测试先于实现

PTY 的单元测试在此环境中可运行，但它们不能完全模拟真实终端的复杂行为（bashrc 差异、PROMPT_COMMAND 变化等）。集成测试应该在真实终端环境中进行。
