# NaShell 实现说明

本文档定义项目实现中的代码风格、架构规范、数据结构，以及核心数据流。所有实现必须严格遵循此文档的约束。

---

## 一、代码风格

### 1.1 文件组织

- 每个 Rust 源文件不超过 500 行。超过必须拆分。
- 每个结构体 / trait / enum 独立一个文件，同类型文件放入同一文件夹。
- 例如：
  ```
  src/
  ├── main.rs                       # 程序入口
  ├── repl/
  │   ├── mod.rs                    # REPL 循环
  │   ├── input.rs                  # 多行输入收集
  │   └── prompt.rs                 # 提示符渲染
  ├── parser/
  │   ├── mod.rs                    # 解析器入口
  │   ├── lexer.rs                  # 词法分析
  │   ├── syntax.rs                 # RawCommands / RawCmd / CmdType 结构体
  │   ├── long_arg.rs               # long_argument 提取
  │   └── pipeline.rs               # 管道分割
  ├── executor/
  │   ├── mod.rs                    # 执行引擎分派
  │   ├── shell_exec.rs             # Shell 命令执行
  │   ├── nacommand_exec.rs         # NaCommand 执行
  │   ├── interactive_exec.rs       # 交互命令执行
  │   └── pipeline_orch.rs          # 管道编排
  ├── nacommand/
  │   ├── mod.rs                    # NaCommand 模块入口
  │   ├── cmd.rs                    # NaCommand / NaLevel 结构体
  │   ├── registry.rs               # 命令注册与查表
  │   ├── builtin/
  │   │   ├── mod.rs
  │   │   ├── write.rs              # Write 命令
  │   │   ├── open.rs               # Open 命令
  │   │   ├── bash.rs               # Bash 命令
  │   │   └── shell_cmd.rs          # Shell 管理命令
  │   └── external.rs               # 用户配置的外部命令执行
  ├── shell/
  │   ├── mod.rs                    # Shell 管理入口
  │   ├── actor.rs                  # Shell 结构体与 ShellActor
  │   ├── cmd.rs                    # ShellCmd 枚举
  │   ├── out.rs                    # ShellOut 枚举
  │   ├── pty.rs                    # PTY 创建与管理
  │   ├── cwd_sync.rs               # cwd 同步
  │   └── manager.rs                # Shell 管理器（main + async shells）
  ├── plugin/
  │   ├── mod.rs                    # 插件系统入口
  │   ├── protocol.rs               # 消息类型定义（call/response/off/broadcast）
  │   ├── manager.rs                # 插件进程管理与保活
  │   ├── manifest.rs               # manifest.json 解析
  │   ├── toexec.rs                 # toExec 递归执行与深度控制
  │   └── broadcast.rs              # broadcast 广播
  ├── config/
  │   ├── mod.rs                    # 配置模块入口
  │   ├── loader.rs                 # KDL 配置加载
  │   ├── schema.rs                 # 配置数据结构
  │   └── alias.rs                  # Alias 解析与展开
  ├── app/
  │   ├── mod.rs                    # AppData 结构体
  │   └── init.rs                   # 程序初始化流程
  └── error/
      ├── mod.rs                    # 错误类型定义
      └── display.rs                # 错误显示格式
  ```

- 不同职责的代码禁止放入同一个文件。
- 同类功能必须归入同一个文件夹（如所有解析相关代码在 `parser/` 下）。

### 1.2 注释要求

- **每个结构体、枚举、trait、方法、函数必须有文档注释**（`///`）。
- 文档注释格式：
  ```rust
  /// 简述功能（一行）。
  ///
  /// 详细说明（如有必要）。
  ///
  /// # 参数
  /// - `param_name`: 参数说明
  ///
  /// # 返回
  /// 返回值说明
  ///
  /// # 错误
  /// 可能的错误场景
  ///
  /// # 示例
  /// ```rust
  /// // 使用示例
  /// ```
  ```
- 可以使用行注释 `//` 解释复杂逻辑，但不应泛滥。优先让代码自解释。

### 1.3 禁止事项

| 禁止 | 说明 | 替代方式 |
|------|------|---------|
| `unwrap()` / `expect()` | 不允许在生产代码中使用 | 必须用 `?` 或 `match` 处理所有 `Option`/`Result` |
| 魔法数字 | 任何裸数字常量 | 定义为 `const` 或有意义命名的常量 |
| 过长方法 | 单个方法不超过 60 行 | 拆分为多个私有辅助方法 |
| 裸 `panic!` | 不允许主动 panic | 返回 `Result::Err` |
| `unsafe` 代码 | 除非绝对必要 | 需要时必须在注释中说明理由 |
| `clone()` 滥用 | 避免不必要的克隆 | 使用引用或 `Cow` |
| 大写缩写命名 | 如 `HTTPClient` → `HttpClient` | 遵循 Rust 命名惯例 |

### 1.4 错误处理

- 所有可失败的函数返回 `Result<T, NashellError>`。
- 定义统一的错误类型 `NashellError`，包含足够的上下文信息。
- 错误类型结构参考：
  ```rust
  /// NaShell 统一错误类型
  pub enum NashellError {
      /// 解析错误
      Parse { context: String, detail: String },
      /// 执行错误
      Execute { command: String, exit_code: Option<i32>, stderr: String },
      /// 配置错误
      Config { path: String, detail: String },
      /// IO 错误
      Io { path: Option<String>, source: std::io::Error },
      /// 插件错误
      Plugin { plugin_name: String, detail: String },
      /// 命令未找到
      CommandNotFound { name: String },
      /// 超时
      Timeout { command: String, seconds: u64 },
      /// 安全拦截
      SafetyBlocked { command: String, reason: String },
  }
  ```

### 1.5 日志

- 使用 `log` crate（配合 `env_logger` 或类似后端）。
- 日志级别使用规范：
  - `error!`: 不可恢复的错误（但程序不退出）
  - `warn!`: 可恢复的异常情况（如插件超时重试）
  - `info!`: 重要的状态变更（如 shell 切换、插件启动）
  - `debug!`: 调试信息（命令执行细节）
  - `trace!`: 非常详细的追踪（每个解析步骤）

---

## 二、核心数据结构

以下为不可变的数据结构定义。实现时必须严格遵循字段名和类型。

### 2.1 解析层

位置：`src/parser/syntax.rs`

```rust
/// 解析后的命令集合
pub struct RawCommands {
    /// 按管道分割后的命令列表
    pub commands: Vec<RawCmd>,
    /// @/ 或空行后的长参数内容
    pub long_argument: Option<String>,
    /// 前一个命令的管道输出（执行时填充）
    pub pre_out: Option<String>,
    /// 异步执行的目标 shell 名称，None 表示同步
    pub async_name: Option<String>,
}

/// 单个命令的解析结果
pub struct RawCmd {
    /// 命令类型
    pub cmd_type: CmdType,
    /// 命令本体（如 "ls", "Write", "hx"）
    pub cmd: String,
    /// 命令行参数
    pub args: Vec<String>,
}

/// 命令类型枚举
pub enum CmdType {
    /// 普通 shell 命令（无特殊前缀）
    Shell,
    /// 交互式命令（!cmd 前缀）
    Interactive,
    /// 普通 NaCommand（!@Cmd: 前缀）
    NaCommandNormal,
    /// 系统级 NaCommand（!!@Cmd: 前缀）
    NaCommandSystem,
}
```

### 2.2 NaCommand 层

位置：`src/nacommand/cmd.rs`

```rust
/// NaCommand 执行时的数据结构
pub struct NaCommand {
    /// 命令级别
    pub level: NaLevel,
    /// 命令名（小写）
    pub cmd: String,
    /// 子命令/模式（如 "watch", "help"），None 表示默认模式
    pub mode: Option<String>,
    /// 选项参数（如 ["-q", "rust", "-c", "10"]）
    pub args: Vec<String>,
    /// 多行长参数
    pub long_argument: Option<String>,
}

/// 命令级别
pub enum NaLevel {
    Normal,
    System,
}
```

### 2.3 Shell 管理

位置：`src/shell/actor.rs`、`src/shell/cmd.rs`、`src/shell/out.rs`

```rust
/// 一个持久的 Shell 线程
pub struct Shell {
    /// Shell 名称（"main" 为当前主 shell）
    pub name: String,
    /// 随机分配的唯一 id
    pub id: String,
    /// 工作路径（通过 PTY 同步）
    pub path: PathBuf,
    /// 命令接收端
    pub cmd_rx: Receiver<ShellCmd>,
    /// 输出发送端
    pub out_tx: Sender<ShellOut>,
    /// 执行输出池（用于非 main shell 的异步执行结果积累）
    pub pools: Vec<String>,
}

/// 发给 Shell 线程的命令
pub enum ShellCmd {
    /// 在 PTY 中直接执行（实时输出）
    ExecPty { input: String },
    /// 通过 -c 捕获执行
    ExecCaptured { cmd: String, args: Vec<String> },
    /// 切换为 main shell
    Switch(String),
    /// 中断当前执行
    Stop,
    /// 销毁线程
    Destroy,
    /// 查看 pools 中最近 count 条输出
    Watch { count: usize },
    /// 获取状态快照
    GetState,
}

/// Shell 线程的输出
pub enum ShellOut {
    /// PTY 实时输出块
    Working(String),
    /// -c 模式捕获完毕
    Captured { stdout: String, stderr: String, exit_code: i32 },
    /// 命令执行完毕（PTY 模式）
    Wait,
    /// 确认已销毁
    Destroyed,
    /// 切换结果
    Switched { new_name: String, id: String },
    /// 状态快照
    State { name: String, id: String, path: String, pools_count: usize },
}
```

### 2.4 应用全局状态

位置：`src/app/mod.rs`

```rust
/// 程序运行时全局数据
pub struct AppData {
    /// 内置命令注册表
    pub builtin_cmds: Vec<CmdMeta>,
    /// 用户配置的外部 NaCommand
    pub config_cmds: Vec<CmdMeta>,
    /// 插件注册表
    pub plugins: Vec<PluginMeta>,
}

/// 命令元数据（内置/外部配置/插件共享）
pub struct CmdMeta {
    pub level: Level,
    pub name: String,
    pub exec: String,
    pub long_argument: bool,
    pub exec_script: Option<String>,
}

/// 插件元数据
pub struct PluginMeta {
    pub name: String,
    pub exec: String,
    pub is_broadcast: bool,
    pub commands: Vec<CmdMeta>,
}

/// 命令级别（与 NaLevel 对应但用于查表阶段）
pub enum Level {
    Normal,
    System,
}
```

### 2.5 插件协议

位置：`src/plugin/protocol.rs`

```rust
/// 插件消息的 type 字段
pub enum PluginMsgType {
    /// 调用命令
    Call,
    /// 流式/分批响应
    Response,
    /// 结束通知
    Off,
    /// 广播消息
    Broadcast,
}

/// 主程序发给插件的 call 消息
pub struct PluginCall {
    pub command: String,
    pub mode: String,
    pub level: String,
    pub params: Vec<String>,
    pub long_argument: Option<String>,
}

/// 插件发给主程序的 response 消息
pub struct PluginResponse {
    pub streaming: bool,
    pub out_content: String,
    pub out_prompt: Option<String>,
    pub is_print: bool,
    pub to_exec: Vec<String>,
    pub exec_result: Option<Vec<String>>,
}

/// 插件发给主程序的 off 消息
pub struct PluginOff {
    pub to_exec: Vec<String>,
    pub out_content: String,
    pub out_prompt: Option<String>,
    pub is_print: bool,
}

/// 主程序广播消息
pub struct PluginBroadcast {
    pub event: String,
    pub payload: serde_json::Value,
}

/// 最大 toExec 递归深度
pub const TOEXEC_MAX_DEPTH: u32 = 3;
```

### 2.6 配置

位置：`src/config/schema.rs`

```rust
/// 完整配置
pub struct NashellConfig {
    pub opening: OpeningConfig,
    pub prompts: PromptsConfig,
    pub na_commands: HashMap<String, ExternalCmdConfig>,
    pub aliases: HashMap<String, String>,
    pub shell: ShellConfig,
    pub safety: SafetyConfig,
    pub plugins: PluginsConfig,
}

pub struct OpeningConfig {
    pub exec: Option<String>,
    pub file: Option<String>,
}

pub struct PromptsConfig {
    pub input_prompt_fg: String,
    pub input_prompt_format: String,
    pub input_continue_format: String,
    pub output_prompt_format: String,
    pub output_prompt_fg: String,
    pub bash_output_prompt_fg: String,
    pub shell_type_fg: String,
}

pub struct ExternalCmdConfig {
    pub exec: String,
    pub long_argument: bool,
    pub exec_script: Option<String>,
}

pub struct ShellConfig {
    pub timeout_secs: u64,
}

pub struct SafetyConfig {
    pub deny_patterns: Vec<String>,
}

pub struct PluginsConfig {
    pub dir: String,
    pub max_recursion_depth: u32,
}
```

---

## 三、核心数据流

### 3.1 主循环数据流

```
用户输入 (String)
    │
    ▼
┌──────────────────────┐
│  REPL 多行输入收集     │  src/repl/input.rs
│  收集首行 + >> 行      │
└──────────┬───────────┘
           │ 完整输入 String
           ▼
┌──────────────────────┐
│  词法分析 (Lexer)      │  src/parser/lexer.rs
│  识别前缀/截止符/管道   │
└──────────┬───────────┘
           │ Token 流
           ▼
┌──────────────────────┐
│  语法分析 (Parser)     │  src/parser/mod.rs
│  RawCommands 结构体    │
└──────────┬───────────┘
           │ RawCommands
           ▼
┌──────────────────────┐
│  模式判定              │  src/repl/mod.rs
│  should_use_direct()  │
└──────┬───────┬───────┘
       │       │
  直连模式   Captured 模式
       │       │
       ▼       ▼
┌──────────┐ ┌──────────────────┐
│dispatch  │ │ dispatch()       │
│_direct() │ │ 遍历 RawCmd 逐段  │
│          │ │ 管道前段输出→后段  │
│{shell}   │ │                  │
│ -c exec  │ │ script -e -q -c  │
│(stdio   │ │ 捕获 stdout/err   │
│ inherit) │ │                  │
└────┬─────┘ └────────┬─────────┘
     │                │
     ▼                ▼
┌──────────────────────┐
│  输出打印              │  src/repl/mod.rs
│  带提示符/ANSI 码      │  print_shell_prefix()
│                      │  print_captured_output()
└──────────┬───────────┘
           │
           ▼
      下一个 REPL 循环
```

### 3.2 解析流程（阶段 0~4）

见 `docs/nashell_dev.md` 的 "执行流（从输入到提交）" 章节，阶段 0~4。

实现文件：
- 阶段 0 (输入收集): `src/repl/input.rs`
- 阶段 1 (!!@Bash: 检测): `src/parser/lexer.rs` — `detect_bash_shortcut()`
- 阶段 2 (@/Async 检测): `src/parser/lexer.rs` — `detect_async_marker()`
- 阶段 3 (long_argument 提取): `src/parser/long_arg.rs`
- 阶段 4 (管道分割): `src/parser/pipeline.rs`

### 3.3 Shell 命令执行流

NaShell 采用**双模式执行架构**，根据命令特征自动选择执行方式：

#### 直连模式（`exec_shell_direct`）

条件：单一命令 ∧ 无管道 `|` ∧ 无 `@/Async` ∧ 非 `!!@Bash:`

```
Rust Command → {shell} -c '{command}'  (stdio 全部 inherit)
```

- stdin/stdout/stderr 全部继承自父进程，子进程直接读写真实终端
- 适用于实时输出（进度条、git clone）和交互输入（python REPL、read、TUI）
- `cd` 命令不经过此路径，由 Rust 进程直接拦截

#### Captured 模式（`exec_captured`）

条件：有管道 `|` / 有 `@/Async` / `!!@Bash:`

```
Rust Command → script -e -q -c "{shell} -c '{command}'" /dev/null
```

- `script` 分配伪终端，命令感知到 TTY 后正常输出（解决 eza、ls --color=auto 等工具的 TTY 检测问题）
- 使用 `Stdio::inherit()` 将真实终端 stdin 传给 `script`，确保 PTY 尺寸正确（否则 nushell 表格报 0 columns）
- `script -e` 使退出码正确传递
- stdout/stderr 通过 pipe 捕获为 String，用于管道传递或格式化输出
- 支持超时机制：超时后发送 SIGTERM → SIGKILL 终止

#### Bash 命令（`exec_bash`）

`!!@Bash:` 专用路径：

```
Rust Command → script -e -q -c "bash -c '{args}'" /dev/null
```

- 直接构造 `bash -c` 命令，不经过 `exec_captured` 的 `{shell} -c` 双层包装
- 输出使用亮黄色 `Bash:` 标识（`bash_output_prompt_fg`）

#### 安全拦截

所有命令在执行前通过 `check_safety()` 检查，匹配 `safety.deny_patterns` 中的任一模式则拒绝执行。

#### 执行分派

REPL 循环通过 `should_use_direct()` 判定模式：
- 直连模式 → `dispatch_direct()`（仅 Shell/Interactive 类型）
- Captured 模式 → `dispatch()`（所有类型，管道逐段执行）

实现文件：
- 执行核心: `src/executor/shell_exec.rs` — `exec_captured()`、`exec_shell_direct()`、`exec_bash()`、`exec_cd()`、`shell_quote()`、`wait_child_with_timeout()`
- 执行分派: `src/executor/mod.rs` — `dispatch()`、`dispatch_direct()`、`check_safety()`
- 模式判断: `src/repl/mod.rs` — `should_use_direct()`、`print_shell_prefix()`、`print_captured_output()`
- PTY 基础设施（后续用）: `src/shell/pty.rs`
- 提示符颜色: `src/repl/prompt.rs` — `colorize()`、`ansi_code()`

### 3.4 NaCommand 执行流

见 `docs/nashell_dev.md` 的 "NaCommand" 章节 和 "完整执行流" 章节。

查表逻辑：
1. 查 `AppData.builtin_cmds` → 匹配则调用内置 handler
2. 查 `AppData.config_cmds` → 匹配则调用外部程序
3. 查 `AppData.plugins[*].commands` → 匹配则发送 call 消息

实现文件：
- 查表与分派: `src/nacommand/registry.rs`
- 内置命令: `src/nacommand/builtin/*.rs`
- 外部命令: `src/nacommand/external.rs`
- 插件调用: `src/plugin/manager.rs`

### 3.5 插件通信流

见 `docs/nashell_dev.md` 的 "插件系统" 章节。

实现文件：
- 通信协议消息定义: `src/plugin/protocol.rs`
- 子进程管理: `src/plugin/manager.rs`
- toExec 递归: `src/plugin/toexec.rs`
- broadcast: `src/plugin/broadcast.rs`

通信时序：
```
NaShell                     Plugin
  │                           │
  │── call (NDJSON line) ──→ │
  │                           │ (处理)
  │←── response #1 ──────── │
  │    (is_print: true,       │
  │     to_exec: [...])       │
  │                           │
  │ (执行 to_exec 命令)        │
  │                           │
  │── response (含 exec_result) →│
  │                           │ (继续处理)
  │←── off ──────────────── │
  │ (关闭插件进程)              │
```

---

## 四、主要框架与依赖

### 4.1 核心依赖

| Crate | 用途 |
|-------|------|
| `portable-pty` | PTY 伪终端创建与管理 |
| `tokio` | 异步运行时（async shell、插件 I/O） |
| `kdl-rs` | KDL 配置文件解析 |
| `serde` + `serde_json` | JSON 序列化（插件通信、配置） |
| `log` + `env_logger` | 日志系统 |
| `rustyline` | REPL 输入行编辑（历史、光标） |
| `nix` | Unix 信号处理（SIGWINCH 等） |
| `regex` | 安全拦截模式匹配 |
| `syntect` | 文件内容的语法高亮（Open 命令） |
| `dirs` | 获取 `~/.config` 等系统路径 |

### 4.2 线程模型

```
主线程 (REPL + 输入/输出)
  │
  ├─ ShellActor 线程 (main PTY)
  │   └─ 持有 PTY master fd，转发 stdin/stdout
  │
  ├─ ShellActor 线程 × N (异步 shell，每个 @/Async(name) 一个)
  │   └─ 独立的 PTY + 独立的命令解析能力
  │
  ├─ Plugin 子进程 × M (通过 stdin/stdout NDJSON 通信)
  │   └─ tokio::spawn 管理其 I/O
  │
  └─ Broadcast 监听器 (通知所有 is_broadcast 插件)
```

### 4.3 channel 通信

| Channel | 发送方 | 接收方 | 用途 |
|---------|--------|--------|------|
| `cmd_tx` / `cmd_rx` | 主线程 | ShellActor | 发送 ShellCmd |
| `out_tx` / `out_rx` | ShellActor | 主线程 | 接收 ShellOut |
| 插件 stdin | 主线程 | 插件进程 | 发送 call/response/broadcast |
| 插件 stdout | 插件进程 | 主线程 | 接收 response/off |

---

## 五、常量和默认值

所有常量集中定义在 `src/constants.rs`。

```rust
/// 文件读取默认行数限制
pub const DEFAULT_OPEN_LIMIT: usize = 500;

/// 文件读取最大行数
pub const MAX_OPEN_LIMIT: usize = 2000;

/// Shell 默认超时（秒）
pub const DEFAULT_SHELL_TIMEOUT_SECS: u64 = 120;

/// 插件通信默认超时（秒）
pub const PLUGIN_TIMEOUT_SECS: u64 = 30;

/// toExec 最大递归深度
pub const TOEXEC_MAX_DEPTH: u32 = 3;

/// 管道输出默认最大字节数（不截断时为 0）
pub const MAX_PIPE_BUFFER_BYTES: usize = 0;

/// PTY 窗口默认列数
pub const DEFAULT_PTY_COLS: u16 = 80;

/// PTY 窗口默认行数
pub const DEFAULT_PTY_ROWS: u16 = 24;

/// cwd 轮询间隔（毫秒）
pub const CWD_POLL_INTERVAL_MS: u64 = 200;

/// 配置文件默认路径（相对于 home）
pub const DEFAULT_CONFIG_PATH: &str = ".config/nashell/config.kdl";

/// 插件默认目录（相对于 home）
pub const DEFAULT_PLUGINS_DIR: &str = ".config/nashell/plugins";
```

---

## 六、实现注意事项

1. **PTY 输出解析**：从 PTY 读取的输出是带 ANSI 转义序列的原始字节流。直接透传给用户终端（不要做任何 strip 或转换）。

2. **PTY 退出检测**：通过 `waitpid` 或 PTY master 的 EOF 检测 shell 子进程退出。若 main PTY 异常退出，尝试自动重启。

3. **管道捕获模式下的 ANSI 码**：`-c` 捕获的输出可能包含 ANSI 码（如 `ls --color=always`）。保留这些码，完整传递。

4. **插件通信的 NDJSON**：每行一条完整的 JSON，以 `\n` 结尾。解析时按行读取，空行跳过。JSON 内部不得包含未转义的换行符。

5. **`!!@Bash:` 的 `@/Async`**：Bash 命令检测发生在所有其他解析之前。`!!@Bash:` 和其后的 `@/Async(name)` 同时被 Bash 处理器消费，不经过常规的 `@/Async` 检测阶段。

6. **`!cmd` 的 sudo 处理**：先通过 PTY 执行 `sudo -v`（刷新 sudo 时间戳），然后 exec 带 sudo 的目标交互程序。

7. **错误显示格式**：所有错误输出统一为 `@Error #>>\n{错误类型}: {描述}` 格式，红色前景色。

8. **配置文件缺失处理**：若 `~/.config/nashell/config.kdl` 不存在，使用内置默认值启动，不报错。若存在但解析失败，报告解析错误但继续使用默认值启动。

9. **大小写匹配**：所有 NaCommand 命令名和模式名在匹配时统一转小写。用户输入 `Open`、`OPEN`、`open` 均匹配为 `open`。

10. **exec_script 临时文件**：临时脚本文件创建在 `/tmp/nashell/` 下，文件名包含随机串避免冲突。执行结束后无论成功与否都删除。
