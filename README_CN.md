# NaShell

基于现有 Shell（nushell/bash）之上的"伪 Shell"，为人类用户和 LLM Agent 共用而设计。
核心特色是**语义级特殊命令**（`NaCommand`），它们与传统 Shell 命令无缝兼容。

## 核心设计

- **语义级 NaCommand** — 结构化命令：类型化参数、长文本输入、模式分派。通过 `@/` 截止符实现免转义的多行输入。
- **Shell 兼容** — 所有标准 Shell 命令（`ls`、`git`、`curl`、`vim` 等）通过底层 Shell 原生执行。
- **双模执行** — 直连终端模式（交互式 TUI 程序）、Captured PTY 模式（管道传递和格式化输出）。
- **插件系统** — 基于 NDJSON 的通信协议，支持流式输出、toExec 委托、广播事件。
- **异步 Shell** — `@/Async(name)` 后台执行，独立 Shell 环境，输出积累到 pools。

## 快速开始

```bash
# 编译
cargo build --release

# 运行
./target/release/nashell
```

```text
~/projects/nashell |> ls -la                   # Shell 命令——原生执行
~/projects/nashell |> ls | grep src            # 管道也可用
~/projects/nashell |> !@Write:./hello.py @/     # NaCommand: 写入文件
                     >> print("你好，NaShell！")
                     >>
~/projects/nashell |> !!@Bash: ls -la           # Bash 快捷方式
~/projects/nashell |> echo done @/Async(back)   # 异步执行
~/projects/nashell |> !!@Shell:                 # Shell 管理
```

## 架构总览

```
NaShell 进程
├─ REPL 前端 (rustyline)
│   ├─ 多行输入（@/ 截止符）
│   ├─ ANSI 彩色提示符
│   └─ 历史管理
├─ 命令解析器
│   ├─ 词法分析: 识别 !@/!!@/@//管道/引号字符串
│   ├─ 异步标记检测 (@/Async(name))
│   ├─ long_argument 提取 (@/ 或空行)
│   └─ 管道分割 (|)
├─ 执行引擎
│   ├─ 直连模式: stdin/stdout/stderr inherit → 交互式程序
│   ├─ Captured 模式: script -e -q -c → PTY 感知输出捕获
│   ├─ NaCommand 分派: 内置 → 配置 → 插件 三级查表
│   └─ 安全拦截: deny_patterns 匹配
├─ Shell 管理器
│   ├─ 主 Shell（cwd 与 Rust 进程同步）
│   ├─ 异步 Shell（后台线程，独立工作目录）
│   └─ Shell pools（异步执行输出积累）
├─ 插件管理器
│   ├─ 子进程生命周期（启动/发送/接收/关闭）
│   ├─ stdin/stdout NDJSON 帧协议
│   ├─ toExec 递归执行引擎（深度限制）
│   └─ 广播事件通道
└─ 配置加载器
    ├─ ~/.config/nashell/config.kdl (KDL 格式)
    ├─ ~/.config/nashell/plugins/ (manifest.json 扫描)
    └─ 别名展开
```

## 内置 NaCommand

| 命令 | 级别 | 说明 |
|---------|-------|------|
| `!@Write:` | Normal | 写入文件。路径来自参数，内容来自 `@/` 后的 long_argument。 |
| `!@Open:` | Normal | 打开文件或目录。文件模式含语法高亮，目录模式显示结构树。 |
| `!!@Bash:` | System | 通过 `bash -c` 执行。解析优先级最高，跳过其他所有规则。 |
| `!!@Shell:` | System | 管理 Shell 线程。模式：默认、Watch、Destroy、Switch。 |

## 插件系统

插件是独立可执行程序，通过 stdin/stdout 以 NDJSON 格式通信。

详见 **[插件开发指南](docs/plugin_dev.md)**。

```json
// manifest.json
{
    "name": "my_plugin",
    "exec": "python3 /path/to/plugin.py",
    "nacommands": {
        "hello": { "level": "normal", "long_argument": true }
    },
    "is_broadcast": false
}
```

核心特性：
- **语言无关** — 任何支持 JSON + stdin/stdout 的语言均可
- **流式输出** — 多段实时响应，逐条打印
- **toExec 委托** — 插件请求主程序代为执行命令
- **广播事件** — 订阅 `cwd_changed` / `shell_state_changed` 事件

## 配置

配置文件：`~/.config/nashell/config.kdl`（KDL 格式）。缺失时使用内置默认值。

```kdl
opening { exec "fastfetch" }

prompts {
    input_prompt_fg "green"
    input_prompt_format "{path} |> "
    input_continue_format ">> "
    output_prompt_format "@System #>>"
    output_prompt_fg "gray"
    bash_output_prompt_fg "bright_yellow"
    shell_type_fg "blue"
}

NaCommands {
    edit exec="n_edit" long_argument=true exec_script=".ned"
    websearch exec="nu ./web_search.nu" long_argument=false
}

alias {
    ll "ls -la"
    gst "git status"
}

shell { timeout_secs 120 }

safety {
    deny_patterns [
        "sudo ", "rm -rf /", "rm -rf /*",
        "chmod 777 /", "dd if=", "> /dev/sda"
    ]
}

plugins {
    dir "~/.config/nashell/plugins"
    max_recursion_depth 3
}
```

## 输入语法

```
// Shell 命令——传给 nushell/bash 执行
ls -la

// NaCommand（Normal 级）
!@Write:./path @/
>> 多行内容直接写在这里
>> 无需转义引号等特殊字符

// NaCommand（System 级）
!!@Shell:Watch -i abc123 -c 3

// Bash 快捷方式
!!@Bash: ls -la

// 管道
ls | grep Cargo | !@Write:./output.txt @/

// 异步执行
echo hello @/Async(my_shell)

// 别名
ll    // → ls -la
gst   // → git status
```

## 执行流

```
用户输入 → 别名展开 → 词法分析 → 语法分析 → RawCommands
                                                │
                         ┌──────────────────────┤
                         ↓                      ↓
                   异步执行 (@/Async)        同步执行
                         │                      │
                 后台线程 spawn → pools    should_use_direct?
                                                │
                               ┌─────────────────┤
                               ↓                 ↓
                          直连模式           Captured 模式
                         （交互式程序）      （管道/格式化输出）
                               │                 │
                         Stdio::inherit    script -e -q -c
                         cd 由 Rust 拦截   dispatch() 管道编排
```

## 项目结构

```
src/
├── main.rs              # 入口，初始化，启动 REPL
├── constants.rs         # 全部命名常量
├── repl/
│   ├── mod.rs           # REPL 循环、模式判定、broadcast
│   ├── input.rs         # 多行输入收集
│   └── prompt.rs        # ANSI 彩色提示符渲染
├── parser/
│   ├── mod.rs           # 解析入口: 字符串 → RawCommands
│   ├── lexer.rs         # 词法分析: 前缀、管道、引号
│   ├── syntax.rs        # RawCommands/RawCmd/CmdType 结构体
│   ├── long_arg.rs      # @/ 和空行的 long_argument 提取
│   └── pipeline.rs      # 管道分割（引号安全）
├── executor/
│   ├── mod.rs           # 分派引擎、安全拦截、build_nacommand
│   ├── shell_exec.rs    # exec_captured/exec_shell_direct/exec_bash/exec_cd
│   └── async_exec.rs    # 异步执行: 后台完整 parse→dispatch
├── nacommand/
│   ├── mod.rs           # NaCommand 执行: 内置/插件分派
│   ├── cmd.rs           # NaCommand/NaLevel 结构体
│   ├── registry.rs      # 命令注册表、查表、帮助
│   └── builtin/
│       ├── write.rs     # Write 命令
│       ├── open.rs      # Open 命令（语法高亮）
│       ├── bash.rs      # Bash 命令 (!!@Bash:)
│       └── shell_cmd.rs # Shell 管理 (!!@Shell:)
├── shell/
│   ├── actor.rs         # Shell 结构体
│   ├── cmd.rs           # ShellCmd 枚举
│   ├── out.rs           # ShellOut 枚举
│   ├── pty.rs           # PTY 会话管理
│   ├── cwd_sync.rs      # CWD 同步
│   └── manager.rs       # ShellManager: 主 Shell + 异步 Shell
├── plugin/
│   ├── mod.rs
│   ├── protocol.rs      # 消息类型: Call/Response/Off/Broadcast
│   ├── manifest.rs      # manifest.json 加载与扫描
│   ├── manager.rs       # PluginManager: 进程生命周期
│   ├── toexec.rs        # toExec 递归引擎
│   └── broadcast.rs     # 广播事件分发
├── config/
│   ├── mod.rs
│   ├── loader.rs        # KDL 配置加载
│   ├── schema.rs        # 配置数据结构
│   └── alias.rs         # 别名展开
├── app/
│   ├── mod.rs           # AppData/CmdMeta/PluginMeta 结构体
│   └── init.rs          # Shell 类型检测
└── error/
    ├── mod.rs           # NashellError 统一错误类型
    └── display.rs       # 错误格式化输出
```

## 依赖

| Crate | 用途 |
|-------|------|
| `rustyline` | REPL 行编辑与历史 |
| `kdl-rs` | KDL 配置文件解析 |
| `serde` + `serde_json` | JSON 序列化（配置、插件通信） |
| `syntect` | Open 命令语法高亮 |
| `portable-pty` | PTY 伪终端管理 |
| `libc` | Unix 信号处理 |
| `tokio` | 异步运行时（后续阶段） |
| `log` + `env_logger` | 日志系统 |
| `dirs` | 系统目录路径 |

## 开发

```bash
# 运行全部测试（284 个）
cargo test

# 代码检查
cargo clippy

# 带调试日志运行
RUST_LOG=debug cargo run

# 指定配置文件运行
NASHELL_CONFIG=/path/to/config.kdl cargo run
```

## 相关文档

- [开发设计文档](docs/nashell_dev.md) — 原始设计规格
- [实现说明](docs/INSTRUCTION.md) — 代码风格、架构规范、数据流
- [插件开发指南](docs/plugin_dev.md) — 插件协议与示例
- [实现阶段划分](docs/phases.md) — 分阶段进度

## 设计理念

NaShell 脱胎于 [Ncoding](https://github.com/Mypenfly/Ncoding.git) 和 [Nedit](https://github.com/Mypenfly/Nedit.git) 项目，
核心目标是解决 LLM 场景下的两大痛点：

1. **内容转义问题** — 通过 `@/` 截止符将命令语句与内容区域分离，LLM 可以直接写入包含任意特殊字符的多行文本，无需繁琐的转义处理。
2. **命令语义化** — `NaCommand` 不是简单的 alias，而是有类型系统、参数验证和模式分派的结构化命令，让 LLM 更容易理解和生成正确的命令调用。
