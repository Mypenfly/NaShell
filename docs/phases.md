# NaShell 实现阶段划分

每个阶段包含：
- **目标**：该阶段完成后应达到的状态
- **参考文档**：指向 `nashell_dev.md`（开发文档）和 `INSTRUCTION.md`（实现说明）的章节
- **具体任务**：可逐个完成的子任务
- **验证方式**：如何确认阶段完成

---

## Phase 1: 项目脚手架与配置系统

**目标**：项目可编译运行，能加载并解析配置文件。

### 参考文档

| 内容 | 位置 |
|------|------|
| 项目目标与基本设定 | `nashell_dev.md` → 项目目标、基础设定 |
| 架构总览 | `nashell_dev.md` → 架构总览 |
| 配置文件完整 Schema | `nashell_dev.md` → 配置文件完整 Schema |
| 常量定义 | `INSTRUCTION.md` → 五、常量和默认值 |
| 错误类型 | `INSTRUCTION.md` → 1.4 错误处理 |
| 配置数据结构 | `INSTRUCTION.md` → 2.6 配置 |
| 代码风格 | `INSTRUCTION.md` → 一、代码风格 |

### 任务

1. **初始化 Rust 项目**
   - `cargo init`，配置 `Cargo.toml` 引入核心依赖（`kdl-rs`, `serde`, `serde_json`, `log`, `env_logger`, `dirs`, `tokio`）
   - 配置 `[profile.release]` 优化选项

2. **创建 `src/constants.rs`**
   - 定义所有常量（见 `INSTRUCTION.md` 第五章），每个常量必须带文档注释
   - 禁止魔法数字

3. **创建 `src/error/mod.rs`**
   - 定义 `NashellError` 枚举
   - 实现 `std::fmt::Display` 和 `std::error::Error`
   - 所有 variant 必须有文档注释

4. **创建 `src/config/schema.rs`**
   - 定义 `NashellConfig` 及所有子结构体
   - 每个结构体/字段必须有文档注释
   - 实现 `Default` trait 提供内置默认值

5. **创建 `src/config/loader.rs`**
   - 实现 KDL 文件解析
   - 加载优先级：`NASHELL_CONFIG` 环境变量 → `~/.config/nashell/config.kdl` → 默认值
   - 配置文件不存在时使用默认值，不报错
   - 解析失败时报告错误但继续使用默认值

6. **创建 `src/main.rs`**
   - 初始化日志系统
   - 调用配置加载
   - 打印加载结果（debug 级别），程序退出

### 验证
- `cargo build` 成功
- 创建/修改 `~/.config/nashell/config.kdl` 后运行，日志显示正确的配置值
- 删除配置文件后运行，日志显示使用默认值
- 写入无效 KDL 后运行，显示解析错误但程序不崩溃

---

## Phase 2: 核心数据结构 + REPL 骨架

**目标**：所有数据结构定义完毕，REPL 循环可显示提示符并收集单行输入。

### 参考文档

| 内容 | 位置 |
|------|------|
| 提示符格式 | `nashell_dev.md` → 基础设定 第4~5条 |
| 解析数据结构 | `nashell_dev.md` → 执行流 → 参考的解析后数据结构 |
| NaCommand 数据结构 | `nashell_dev.md` → NaCommand 的分级机制 → 统一数据结构 |
| Shell 管理数据结构 | `nashell_dev.md` → Shell 管理数据结构 |
| AppData 数据结构 | `nashell_dev.md` → 完整执行流 → 初始化阶段 |
| 核心数据结构汇总 | `INSTRUCTION.md` → 二、核心数据结构 |
| 文件组织规范 | `INSTRUCTION.md` → 1.1 文件组织 |
| 注释要求 | `INSTRUCTION.md` → 1.2 注释要求 |

### 任务

1. **创建 `src/parser/syntax.rs`**
   - 定义 `RawCommands`、`RawCmd`、`CmdType`
   - 每个结构体/枚举必须带文档注释

2. **创建 `src/nacommand/cmd.rs`**
   - 定义 `NaCommand`、`NaLevel`
   - 文档注释

3. **创建 `src/shell/cmd.rs` 和 `src/shell/out.rs`**
   - 定义 `ShellCmd`、`ShellOut` 枚举
   - 文档注释

4. **创建 `src/shell/actor.rs`**
   - 定义 `Shell` 结构体（不含 PTY handle，Phase 4 加入）
   - 文档注释

5. **创建 `src/app/mod.rs`**
   - 定义 `AppData`、`CmdMeta`、`PluginMeta`、`Level`
   - 文档注释

6. **创建 `src/config/alias.rs`**
   - 定义 alias 解析逻辑（`HashMap<String, String>` 的简单展开）
   - 函数 `expand_alias(input: &str, aliases: &HashMap<String, String>) -> String`

7. **创建 `src/repl/mod.rs`、`src/repl/input.rs`、`src/repl/prompt.rs`**
   - `prompt.rs`: 根据当前路径生成提示符字符串（格式见 `nashell_dev.md` 基础设定第4条）
   - `input.rs`: 使用 `rustyline` 实现单行输入（多行暂不做）
   - `mod.rs`: REPL 循环骨架——显示提示符 → 读取输入 → 打印（占位逻辑）→ 循环

8. **更新 `src/main.rs`**
   - 构造 `AppData`（目前所有 Vec 为空）
   - 调用 REPL 循环

### 验证
- `cargo build` 成功，`cargo run` 进入 REPL
- 提示符正确显示当前路径和 `|>` 符号
- 输入文本后按 Enter 回显（占位），继续等待输入
- 输入 `exit` 退出程序

---

### Phase 2 复盘要点（Phase 3 开始前必须注意）

以下问题在 Phase 2 审查中发现并已在 Phase 2 修复。进入 Phase 3 时需继续保持：

1. **Editor 必须复用**：`rustyline::DefaultEditor` 在 REPL 循环中只创建一次（`repl/mod.rs` 中持有），通过 `read_line_with_editor` 复用。禁止每次 `read_line` 调用时重新创建，否则历史记录丢失且有 TTY 初始化开销。

2. **配置必须生效**：提示符格式来源为 `PromptsConfig::input_prompt_format`（默认 `"{path} |> "`），通过 `prompt::generate_prompt(cwd, home, format)` 的第三个参数传入。禁止硬编码格式字符串。

3. **错误显示模块已就位**：`src/error/display.rs` 提供了 `format_error(err: &NashellError) -> String`，输出 `@Error #>>\n{类型}: {描述}` 格式。后续所有错误输出必须通过此函数统一格式化。

4. **文件组织持续遵循** `INSTRUCTION.md` 1.1 规范——每个结构体/trait/enum 独立文件，同类功能同一文件夹。

---

## Phase 3: 命令解析器 ✅ 已完成

**目标**：完整解析用户输入为 `RawCommands` 结构体，支持多行输入、`@/` 截止符、管道分割、命令类型识别。

### 参考文档

| 内容 | 位置 |
|------|------|
| 多行输入格式 | `nashell_dev.md` → 基础设定 第4条 |
| 执行流阶段 0~4 | `nashell_dev.md` → 执行流 → 阶段 0 ~ 阶段 4 |
| 解析流程 | `INSTRUCTION.md` → 3.2 解析流程 |
| 文件组织 | `INSTRUCTION.md` → 1.1 文件组织 (parser/) |

### 任务

1. ✅ **创建 `src/parser/lexer.rs`**
   - `tokenize(input: &str) -> Result<Vec<Token>, NashellError>`: 将原始输入转为 token 流
   - Token 类型包括：前缀标记（`!@`/`!!@`/`!`）、命令词、参数、管道 `|`、截止符 `@/`、引号字符串
   - 实现 `detect_bash_shortcut(input: &str) -> Option<String>`: 检测 `!!@Bash:` 前缀，返回 Bash 参数
   - 实现 `detect_async_marker(first_line: &str) -> Option<String>`: 检测 `@/Async(name)`，返回 name
   - 正确处理引号内内容（引号内的 `|`、`@/` 不被识别为分隔符）

2. ✅ **创建 `src/parser/long_arg.rs`**
   - `extract_long_argument(input: &str) -> Result<(String, Option<String>), NashellError>`:
     - 规则 A（优先）：查找 `@/`，之前的内容为命令语句，之后的内容为 long_argument
     - 规则 B（回退）：无 `@/` 时查找首个空行分割
     - 规则 C：两项皆无，long_argument 为 None
   - 返回 `(命令语句部分, long_argument)`

3. ✅ **创建 `src/parser/pipeline.rs`**
   - `split_pipeline(cmd_part: &str) -> Result<Vec<String>, NashellError>`: 按 `|` 分割命令段
   - 保护引号内的 `|`

4. ✅ **创建 `src/parser/mod.rs`**
   - `parse(input: &str) -> Result<RawCommands, NashellError>`: 完整解析流程
   - 整合 lexer → long_arg → pipeline 各步骤
   - 填充 `RawCommands.commands`（识别每个段的 `CmdType`）、`long_argument`、`async_name`

5. ✅ **更新 `src/repl/input.rs`**
   - 实现多行输入：首行 `|>` 提示符，后续行 `>>` 提示符
   - 收集所有行直到用户输入空行（仅回车）或输入 `@/` 后按 Enter 结束输入
   - 多行输入时首行自动检测：若首行末尾有 `@/` → 自动进入多行模式

6. ✅ **更新 REPL 循环**
   - 输入收集后调用 `parse()` 并打印解析结果（debug 级别日志）

### 验证
- [x] 单行输入 `ls -la` → 解析为 1 个 `Shell` 类型命令
- [x] 多行输入 `!@Write:./test.py @/` + 多行内容 → 正确提取 long_argument
- [x] `!@Open:./src -l 200` → 识别为 `NaCommandNormal`，cmd=`Open`（大小写保留，Phase 5 查表时转小写）
- [x] `!!@Shell:Watch -i "abc" -c 3` → 识别为 `NaCommandSystem`，cmd=`Shell`，mode=`Watch` 在 args[0] 中
- [x] `ls | grep foo` → 按管道分割为 2 个 `Shell` 命令段
- [x] `ls | !@Write:./out.txt @/` → 管道分割正确，long_argument 被提取
- [x] 引号内的 `|` 不被分割
- [x] `!!@Bash: ls -la` → Bash shortcut 检测生效，cmd_type=`NaCommandSystem`

测试覆盖：141 个单元测试全部通过，`cargo build` 成功。

---

### Phase 3 复盘要点（Phase 4 开始前必须注意）

以下问题在 Phase 3 审查中发现并已修复。进入 Phase 4 时需继续保持：

1. **`!!@Bash:` 命令类型已修正**：解析器返回 `cmd_type=CmdType::NaCommandSystem`、`cmd="bash"`（小写）。Bash 命令参数在 `args[0]` 中作为原始字符串传递。Phase 6 实现 Bash 命令时注意 `bash_args` 需正确传给 `bash -c`。

2. **`long_argument` 空字符串归一化**：`parse()` 中将 `Some("")` 归一化为 `None`。Phase 5+ 可以信任 `long_argument` 为 `None`（无长参数）或 `Some(non-empty)`（有长参数），无需额外判断空字符串。

3. **NaCommand 命令名大小写**：解析器保留用户输入的原始大小写（如 `"Open"`、`"Shell"`）。Phase 5 的 Registry 查表阶段负责统一转小写匹配（`INSTRUCTION.md` 六.9）。

4. **Mode 提取时机**：`!!@Shell:Watch` 的 mode（`"Watch"`）当前在 `RawCmd.args[0]` 中。Phase 5 需从 `RawCmd` 构建 `NaCommand` 时提取 `mode` 字段。

5. **dead_code 警告可忽略**：当前 24 个 dead_code 警告均来自 Phase 4-6 将使用的数据结构（`NaCommand`、`Shell`、`ShellCmd`、`ShellOut`、`CmdMeta` 等），Phase 4 开始逐步消除。

6. **解析性能**：tokenizer 逐字符处理并分配 `Vec<char>`，当前在小输入下性能足够。Phase 11 可优化为基于 `&str` 切片的零拷贝解析。

7. **文件组织持续遵循** `INSTRUCTION.md` 1.1 规范——parser/ 下 lexer、long_arg、pipeline、syntax、mod 各司其职。

---

## Phase 4: Shell 管理 (PTY) + 执行双模式 ✅ 已完成

**目标**：实现 shell 命令执行，支持持久状态（cd）、TTY 感知输出、实时交互和输出捕获双模式。

**设计决策**：采用**双模式执行架构**——直连模式（Stdio::inherit）与 Captured 模式（`script -e -q -c`）按需切换。

**直连模式**（`exec_shell_direct`）：单一命令、无管道、无 @/Async、非 !!@Bash: 时使用。
- `{shell} -c '{command}'` 全部 stdio 继承父进程，子进程直连真实终端
- 支持实时进度输出（curl、git clone）、交互输入（python REPL、read、TUI 程序）
- `cd` 由 Rust 直接拦截处理，不经过子进程

**Captured 模式**（`exec_captured`）：有管道 / @/Async / !!@Bash: 时使用。
- `script -e -q -c "{shell} -c '{command}'" /dev/null`，stdout/stderr 通过 pipe 捕获
- `script` 分配伪终端解决 eza/ls 等工具的 TTY 检测问题
- 使用 `Stdio::inherit()` 传真实终端 stdin 确保 PTY 尺寸正确（nushell 表格）
- `script -e` 传递退出码，命令结束进程自动退出

**Bash 命令**（`exec_bash`）：
- 直接 `script ... "bash -c '{args}'"` 执行，避免与 `exec_captured` 的内层 `{shell} -c` 嵌套
- 输出使用亮黄色 `Bash:` 标识

额外实现（超出原始 Phase 4 范围）：
- Shell 类型输出标识（`@nu #>>` / `@bash #>>` 带颜色）
- 启动时 opening 显示（`opening.exec` / `opening.file`）
- Shell 命令超时机制（`wait_child_with_timeout`，超时后 SIGTERM→SIGKILL）
- 安全拦截检查（`check_safety`，匹配 `deny_patterns`）
- 输出错误统一格式化（`@Error #>>\n{类型}: {描述}`）

### 参考文档

| 内容 | 位置 |
|------|------|
| `script` 方案说明 | 本文件 Phase 4 设计决策 |
| 直连/Captured 双模式 | `INSTRUCTION.md` → 3.3 Shell 命令执行流 |
| PTY 持久化方案（后续 Phase 再用） | `nashell_dev.md` → 架构总览 → PTY 持久化方案说明 |
| Shell 管理数据结构 | `nashell_dev.md` → Shell 管理数据结构 |
| Shell 数据结构（后续 Phase 用） | `INSTRUCTION.md` → 2.3 Shell 管理 |

### 任务

1. ✅ **创建 `src/executor/shell_exec.rs`**
   - `exec_captured(cmd, args, shell_type, timeout)`: 通过 `script -e -q -c` 在 PTY 中捕获执行
   - `exec_shell_direct(cmd, args, shell_type)`: Stdio::inherit 直连终端执行
   - `exec_bash(bash_args, timeout)`: Bash 命令专用执行（避免双层 shell 嵌套）
   - `exec_cd(args)`: 拦截 cd 命令，通过 `std::env::set_current_dir()` 切换目录
   - `shell_quote(s)`: 单引号转义辅助函数
   - `wait_child_with_timeout(child, label, secs)`: 超时等待 + SIGTERM/SIGKILL 终止
   - `CapturedOutput` 结构体：stdout、stderr、exit_code

2. ✅ **更新 `src/executor/mod.rs`**
   - `dispatch()`: Captured 模式分派（管道 / 异步 / Bash）
   - `dispatch_direct()`: 直连模式分派（Shell 直接执行，cd 拦截）
   - `check_safety()`: 安全模式匹配（pub(crate) 供 REPL 层调用）
   - `ExecContext` 包含 shell_type、pre_out、timeout_secs、deny_patterns

3. ✅ **更新 `src/repl/mod.rs`**
   - `should_use_direct()`: 判定当前命令应使用直连还是 Captured 模式
   - 直连路径：安全检查 → `print_shell_prefix()` → `dispatch_direct()`
   - Captured 路径：逐段 `dispatch()` → `print_captured_output()`（带 shell/Bash 前缀）
   - `show_opening()`: 启动时显示 opening.exec 或 opening.file
   - Editor 复用：`DefaultEditor` 由 REPL 持有，全程复用

4. ✅ **创建 `src/repl/prompt.rs`**
   - `generate_prompt()`: 生成路径提示符（支持 `~` 缩写、`{path}` 格式模板）
   - `colorize()`: ANSI 前景色包装
   - `ansi_code()`: 颜色名到 ANSI 码映射（16 种颜色）

5. ✅ **创建 `src/shell/pty.rs`（保留供后续 Phase）**
   - `spawn_pty_session()`、`send_command()` 完整实现
   - 拆分为 `create_pty_io_pair`、`init_shell_session`、`wait_for_shell_ready` 子函数
   - unsafe 代码已添加合理性注释

6. ✅ **更新 `src/app/init.rs`**
   - `detect_shell_type()`: 检测 `nu` / `bash` 可用性

### 验证
- [x] 启动程序，提示符绿色显示当前路径
- [x] 输入 `ls -la` → 正常显示目录内容（含颜色）
- [x] 输入 `eza` → 正常显示（script 提供 TTY）
- [x] 输入 `nu -c 'ls'` 或 nu 环境下 `ls` → nushell 表格正常渲染
- [x] 输入 `cd /tmp` → 提示符路径更新为 `/tmp`
- [x] 输入 `pwd` → 显示 `/tmp`
- [x] `echo "hello"` → 输出 hello
- [x] 非零退出码命令 → 显示错误但不崩溃，退出码正确传递
- [x] `python3 -c "print(input('? '))"` → 直连模式下交互输入正常
- [x] `python3 test_interact.py` → 流式输出逐帧显示 + 交互输入
- [x] `ls | grep Cargo` → Captured 模式管道正确
- [x] `!!@Bash: ls -la` → Bash: 标识 + 亮黄色前缀
- [x] `rm -rf /` → 安全拦截错误
- [x] 173 个单元测试全部通过

---

### Phase 4 复盘要点（Phase 5 开始前必须注意）

1. **双模式架构**：REPL 循环通过 `should_use_direct()` 判断走直连还是 Captured。直连模式解决实时交互问题，Captured 模式解决管道传递问题。后续添加异步 Shell（Phase 6）时，异步 Shell 内部也适用同样的双模式逻辑。

2. **Bash 命令专用路径**：`exec_bash()` 是专门为 `!!@Bash:` 设计的，直接构造 `bash -c '{args}'` 而不经过 `exec_captured` 的 `{shell} -c` 包装。Phase 6 完善 Bash 命令时，注意此函数已可用。

3. **安全检查已就位**：`check_safety()` 为 `pub(crate)`，REPL 层在直连模式下直接调用，`dispatch()` 在 Captured 模式内调用。后续添加新命令类型时，确保安全检查不被绕过。

4. **`shell_type_fg` 默认值**：已从 `"cyan"` 改为 `"blue"`，与 `nashell_dev.md` 配置示例一致。

5. **持久 PTY 代码保留**：`src/shell/pty.rs` 的持久 PTY 实现（`spawn_pty_session`、`send_command`）完整保留，仅在测试中调用。Phase 6 异步 Shell 可能复用它（但目前打算用一次性 bash 子进程）。

6. **文件组织**：`executor/` 下 `shell_exec.rs` 集中了所有 shell 执行函数（captured/direct/bash/cd/timeout），符合 INSTRUCTION.md 1.1 规范。

---

## Phase 5: NaCommand 执行引擎 ✅ 已完成

**目标**：内置 NaCommand（Write、Open）及 Help 模式可正常工作。

### 参考文档

| 内容 | 位置 |
|------|------|
| Write 命令 | `nashell_dev.md` → Write |
| Open 命令 | `nashell_dev.md` → Open |
| Help 模式 | `nashell_dev.md` → Help 模式 |
| NaCommand 数据结构 | `nashell_dev.md` → NaCommand 的分级机制 |
| 查表逻辑 | `INSTRUCTION.md` → 3.4 NaCommand 执行流 |
| 执行分派 | `INSTRUCTION.md` → 3.1 主循环数据流 (NaCmd 分支) |

### 任务

1. ✅ **创建 `src/nacommand/registry.rs`**
   - `CommandRegistry` 结构体：管理所有注册的命令
   - `register_builtin()`: 注册 Write、Open 等内置命令
   - `lookup(cmd_name: &str) -> Result<&CmdMeta, NashellError>`: 查表（内置 → 配置 → 插件）
   - `get_help(cmd_name: &str, mode: Option<&str>) -> Result<String, NashellError>`: 获取帮助信息

2. ✅ **创建 `src/nacommand/builtin/write.rs`**
   - 实现 Write 命令逻辑（见 `nashell_dev.md` Write 章节）
   - 检查父目录存在性
   - long_argument 为 None 时创建空文件/清空文件
   - 返回格式：`write to {abs_path} ({bytes} bytes)`

3. ✅ **创建 `src/nacommand/builtin/open.rs`**
   - 实现 Open 命令逻辑（见 `nashell_dev.md` Open 章节）
   - 路径为目录：输出目录结构树
   - 路径为文件：按行号输出内容，支持 `--limit`/`--start`/`--end`
   - 目录时传入文件选项参数应报错
   - 语法高亮已实现（使用 `syntect`，主题 `base16-ocean.dark`）

4. ✅ **创建 `src/nacommand/builtin/mod.rs`**
   - 注册 Write、Open 到 CommandRegistry

5. ✅ **创建 `src/nacommand/mod.rs`**
   - `execute_nacommand(cmd: &NaCommand, pre_out: Option<String>, registry: &CommandRegistry) -> Result<String, NashellError>`:
     - 查表找到命令处理器
     - 构建完整 NaCommand（合并 long_argument 和 pre_out）
     - 调用对应 handler

6. ✅ **更新 `src/executor/mod.rs`**
   - `dispatch()` 完善 NaCommandNormal / NaCommandSystem 分支
   - 调用 `nacommand::execute_nacommand()`

### 验证
- [x] `!@Write:./test.txt @/` + 内容 → 文件创建成功
- [x] `!@Write:./test.txt @/` + 多行内容 → 内容正确写入，缩进格式保留
- [x] `!@Write:./nonexistent/file.txt @/` + 内容 → 报错（父目录不存在）
- [x] `!@Open:./src` → 显示目录结构（支持 `-l` 控制递归深度，默认 3）
- [x] `!@Open:./src/main.rs -l 50` → 显示前 50 行（带语法高亮）
- [x] `!@Write:Help` → 显示 Write 命令帮助（带 ANSI 颜色美化）
- [x] `!@Open:Help` → 显示 Open 命令帮助（带 ANSI 颜色美化）

附加实现（超出原始 Phase 5 范围）：
- Open 命令语法高亮（`syntect`，按文件扩展名自动选择语言）
- Open 目录递归深度控制（`--limit/-l` 对目录控制深度，默认 3）
- Help 输出 ANSI 颜色美化（命令名亮青加粗、标题亮蓝、代码绿、警告亮黄）
- `dispatch_direct` 对 NaCommand 的错误信息改进

---

### Phase 5 复盘要点（Phase 6 开始前必须注意）

1. **Mode 提取采用查表法**：NaCommand 本质上是"调用格式特殊的 CLI 工具"。mode 的判定不是依赖启发式规则（如检查是否以 `-` 或 `.` 开头），而是**有表查表**：
   - 每条命令在 `CmdMeta.known_modes` 中声明已知模式（小写）。
   - `build_nacommand` 将 `args[0]` 与 `known_modes` 做大小写不敏感匹配：命中 → 提取为 `NaCommand.mode`；未命中 → 保持为 `arg`。
   - **外部配置命令和插件命令** `known_modes` 为空 → 不做查表，args 原样透传，由其内部自行处理 mode。

2. **`CmdMeta` 新增 `known_modes` 字段**：`Vec<String>` 类型。注册内置命令时必须填写。Phase 7-8 实现外部/插件命令时无需填写此字段。

3. **`build_nacommand` 依赖 registry**：函数签名增加了 `registry: &CommandRegistry` 参数，用于查询命令的 `known_modes`。测试构造时需提供含正确 `known_modes` 的 registry。

4. **`execute_nacommand` 只检查 `cmd.mode`**：不再 fallback 检查 `cmd.args[0]`。因为 mode 提取已在 `build_nacommand` 阶段通过查表完成。

5. **Help 模式统一支持**：所有命令在 `known_modes` 中包含 `"help"` 即自动支持 `!@Cmd:Help` 语法。

6. **Phase 6 的 Shell 命令**：需要在 `CmdMeta.known_modes` 中注册 `["watch", "destroy", "switch"]`，`build_nacommand` 将自动提取。

7. **`pre_out` 管道传递未使用**：`execute_nacommand` 接受 `pre_out: Option<String>` 参数但当前未消费（Phase 5 的 Write/Open 不需要）。Phase 6 的 Bash 命令和管道中段 NaCommand 需要正确使用此参数。代码中已添加 TODO 标记。

8. **Open 命令 `--limit/-l` 重载行为**：
    - 文件模式：`-l` 控制显示行数（与 spec 一致）
    - 目录模式：`-l` 控制递归深度（默认 3，新增行为）
    - `-s`/`-e` 对目录报错（与 spec 一致），`-l` 对目录合法
    - `has_file_only_options` 仅检查 `-s`/`-e`，不再检查 `-l`

9. **语法高亮使用 `syntect`**：依赖 `default-fancy` features，`SyntaxSet`/`ThemeSet` 通过 `OnceLock` 延迟加载并全局复用。测试中 `strip_ansi` 辅助函数用于移除 ANSI 码后进行断言。

10. **`dispatch_direct` 错误信息已改进**：NaCommand 进入直连模式时返回 `NashellError::Execute`（而非 `CommandNotFound`），便于排查 `should_use_direct` 判定逻辑问题。

---

## Phase 6: System 级命令

**目标**：`!!@Bash:` 和 `!!@Shell:` 命令完整可用，`@/Async` 异步执行可用。

### 参考文档

| 内容 | 位置 |
|------|------|
| Bash 命令 | `nashell_dev.md` → Bash |
| Shell 命令（Watch/Destroy/Switch） | `nashell_dev.md` → Shell |
| @/Async 异步执行 | `nashell_dev.md` → @/Async(name) 异步 Shell 执行 |
| Bash shortcut 检测 | `INSTRUCTION.md` → 3.2 解析流程 → 阶段 1 |

### 任务

1. **创建 `src/nacommand/builtin/bash.rs`**
   - 实现 Bash 命令逻辑
   - 参数直接传给 `bash -c`（不经过管道分割）
   - 实时输出，输出开头用亮黄色标记 `Bash:`
   - 支持 `@/Async(name)` 异步执行——创建临时 bash 子进程，不持久化

2. **创建 `src/nacommand/builtin/shell_cmd.rs`**
   - Shell 命令的四种模式实现：
     - 默认（无 mode）：获取所有 Shell 状态，表格输出
     - Watch: 查看指定 shell 的 pools，支持 `-i` / `-c`
     - Destroy: 销毁指定 shell，支持 `-i`
     - Switch: 切换 main shell，支持 `-i` / `-d`

3. **创建 `src/executor/async_exec.rs`**
   - `spawn_async_shell(name: &str, command: &str) -> Result<Shell, NashellError>`:
     - 创建新 PTY shell 线程
     - 在新的 ShellActor 中解析并执行命令
     - 结果写入 pools
     - 返回创建确认信息

4. **更新 `src/parser/lexer.rs` 和 `src/executor/mod.rs`**
   - `@/Async(name)` 的完整链路：解析器标记 `async_name` → 执行器在阶段 7 处理
   - 主命令执行完毕后再启动异步 shell

5. **更新 `src/shell/manager.rs`**
   - 实现 `watch_pools(id: &str, count: usize) -> Result<Vec<String>, NashellError>`
   - 实现 `switch_main(id: &str, destroy_old: bool) -> Result<(), NashellError>`

### 验证
- `!!@Bash: ls -la` → bash 执行，输出带亮黄色 Bash: 标识
- `!!@Shell:` → 显示所有 shell 状态表格
- `!!@Shell:Watch -i "xxx" -c 2` → 显示对应 shell 的最近 2 条 pools
- `!!@Shell:Destroy -i "xxx"` → 销毁成功
- `!!@Shell:Switch -i "xxx" -d` → 切换 main shell 并销毁旧 shell
- `ls -la @/Async(test)` → 异步执行，立即返回确认，pools 中有结果
- `!!@Bash: ls -la @/Async(back)` → Bash 异步执行

### 额外完成项

- **`!cmd` 交互命令移除**：直连模式（Phase 4）已使普通 shell 命令能直接运行 vim/htop 等 TUI 程序，`!cmd` 前缀不再需要。`CmdType::Interactive` 已从代码中删除，相关文档已同步更新。
- **Alias 别名系统**：`expandalias` 已实现首词替换式展开，在 REPL 解析前调用。对普通命令和 NaCommand 均生效。已满足当前需求，不单独设 Phase。

---

## Phase 7: 插件系统 ✅ 已完成

**目标**：完整插件生命周期管理——加载、通信、执行、关闭。

### 参考文档

| 内容 | 位置 |
|------|------|
| 插件通信协议 | `nashell_dev.md` → 插件系统 → 通信协议 |
| 消息类型 | `nashell_dev.md` → 插件系统 → 消息类型 |
| 插件配置与注册 | `nashell_dev.md` → 插件系统 → 插件配置与注册 |
| toExec 递归限制 | `nashell_dev.md` → 插件系统 → toExec 递归限制 |
| 插件协议数据结构 | `INSTRUCTION.md` → 2.5 插件协议 |
| 插件通信流 | `INSTRUCTION.md` → 3.5 插件通信流 |
| 线程模型 | `INSTRUCTION.md` → 4.2 线程模型 |

### 任务

1. **创建 `src/plugin/protocol.rs`**
   - 定义所有消息结构体（`PluginCall`、`PluginResponse`、`PluginOff`、`PluginBroadcast`）
   - 实现 `serde::Serialize` / `serde::Deserialize`
   - `send_message(writer: &mut impl Write, msg: &PluginMessage) -> Result<(), NashellError>`: 序列化并写入一行 NDJSON
   - `recv_message(reader: &mut impl BufRead) -> Result<PluginMessage, NashellError>`: 读取一行并反序列化
   - `PluginMessage` 枚举包含所有消息类型

2. **创建 `src/plugin/manifest.rs`**
   - `load_manifest(path: &Path) -> Result<PluginMeta, NashellError>`: 解析 manifest.json
   - `scan_plugins(dir: &Path) -> Result<Vec<PluginMeta>, NashellError>`: 扫描插件目录

3. **创建 `src/plugin/manager.rs`**
   - `PluginManager` 结构体：管理所有插件进程
   - `start_plugin(meta: &PluginMeta) -> Result<PluginHandle, NashellError>`: 启动插件进程
   - `send_call(handle: &PluginHandle, call: &PluginCall) -> Result<(), NashellError>`: 发送 call 消息
   - `recv_responses(handle: &PluginHandle) -> Result<Vec<PluginResponse>, NashellError>`: 接收 response 直到 off
   - `stop_plugin(handle: &PluginHandle) -> Result<(), NashellError>`: 关闭插件进程
   - 超时处理（`PLUGIN_TIMEOUT_SECS` 秒无响应则强制关闭）

4. **创建 `src/plugin/toexec.rs`**
   - `execute_toplevel(to_exec: &[String], depth: u32, ctx: &ExecContext) -> Result<Vec<String>, NashellError>`:
     - 按顺序逐条执行命令
     - 每条命令走完整解析+执行流程（模拟用户输入）
     - 深度计数：每次 toExec 调用 `depth + 1`
     - 超过 `TOEXEC_MAX_DEPTH` 后：NaCommand 报错拒绝，仅允许纯 shell 命令
   - `ExecContext` 包含当前深度、ShellManager、CommandRegistry 等

5. **创建 `src/plugin/broadcast.rs`**
   - `broadcast_event(event: &str, payload: serde_json::Value, plugins: &[PluginHandle])`: 向所有 is_broadcast 插件发送消息
   - 在主程序的事件触发点调用（如 shell 切换、cwd 变更）

6. **更新 `src/nacommand/registry.rs`**
   - 查表加入插件命令匹配（在配置命令之后，内置命令之后）
   - 匹配到插件命令时 → 调用 `PluginManager::send_call()` → 等待 response/off → 返回结果

7. **更新 `src/app/init.rs`**
   - 启动时启动所有 `is_broadcast: true` 的插件并保活

### 验证
- [x] 创建测试插件（一个简单脚本，接收 NDJSON call，返回 response + off）
- [x] 注册插件命令，调用该命令 → 正确返回结果
- [x] 插件 response 中 `is_print: true` → 内容正确打印
- [x] 插件 `to_exec` 中的 shell 命令 → 正确执行，结果填入 exec_result
- [x] 插件 `to_exec` 递归超过 3 层 → NaCommand 被拒绝
- [x] 插件超时 90 秒无响应 → 强制关闭，报错
- [x] 广播事件 → 所有 is_broadcast 插件收到消息

### 后续完善（Phase 7 后期追加）

以下增强在初始实现之后完成，详见 `plugin_dev.md`：

- **prompt_fg 颜色字段**：PluginResponse / PluginOff 新增 `prompt_fg`，插件可指定输出提示符颜色，默认 `"gray"`
- **GetInput 交互输入**：新增 `get_input` 字段（含 `pre_content` / `input_prompt` / `pre_fg` / `input_fg`），支持插件在运行中向用户请求交互输入，用户提交后通过 `user_input` 回传
- **toExec 直连/流式模式**：Shell 单命令（无管道）改为 `exec_captured_streaming` 实时输出到终端同时捕获；有管道仍走 captured dispatch
- **toExec 结果标注**：每条命令结果前加嵌套提示符 `@[cmd] #>`（dark_gray），区分命令来源
- **broadcast 事件接入**：`cwd_changed` / `shell_state_changed` 已在 REPL 中触发，payload 含完整 shell 状态
- **recv_responses 超时**：看门狗线程 90s（`PLUGIN_RECV_TIMEOUT_SECS`），超时 SIGTERM→SIGKILL
- **demo 插件重写**：`test_plugins/demo_plugin/` 展示 Echo/Stream/Exec/MultiExec/Confirm 全部功能

---

## Phase 8: 外部配置命令 ✅ 已完成

**目标**：用户在 `config.kdl` 中配置的 NaCommand 可正常调用。

### 参考文档

| 内容 | 位置 |
|------|------|
| 用户外部命令配置 | `nashell_dev.md` → 用户外部命令自行配置 |
| NaCommands 配置块 | `nashell_dev.md` → 配置文件完整 Schema → NaCommands |
| 外部命令执行 | `INSTRUCTION.md` → 3.4 NaCommand 执行流 → 外部命令 |

### 任务

1. **创建 `src/nacommand/external.rs`**
   - `execute_external(cmd_meta: &CmdMeta, nacommand: &NaCommand) -> Result<String, NashellError>`:
     - 若无 `exec_script`：long_argument 作为字符串传给 exec 程序的最后一个参数
     - 若有 `exec_script`：long_argument 保存为 `/tmp/nashell/{random}.{ext}` 临时文件，临时文件路径作为 exec 的参数
     - 执行结束后删除临时脚本文件（无论成功与否）
     - 执行 `cmd_meta.exec` 程序，捕获 stdout + stderr，保留 ANSI 码
   - Help 模式：对 exec 程序传入 `--help`，透传输出

2. **更新 `src/nacommand/registry.rs`**
   - 查表加入配置命令匹配（在内置命令之后，插件命令之前）

3. **更新 `src/app/init.rs`**
   - 从配置中加载 `NaCommands` 到 `AppData.config_cmds`

### 验证
- 在 config.kdl 中配置一个 NaCommand，如 `websearch exec="nu ./web_search.nu" long_argument=false`
- 调用 `!@WebSearch: -q "test"` → 正确执行 `nu ./web_search.nu -q test`
- 配置带 exec_script 的命令，验证临时脚本生成和清理
- `!@WebSearch:Help` → 透传 `--help` 输出

### Phase 8 注意事项

> 以下来自 Phase 1-7 积累的经验，进入 Phase 8 时需特别留意：

1. **外部命令也走 toExec 直连逻辑**：若外部命令是不含管道的纯 Shell 调用，应考虑使用 `exec_captured_streaming` 实时输出 + 捕获，与插件 toExec 保持一致。
2. **config_cmds 查表优先级**：内置 → **外部配置** → 插件。当前 `lookup_with_source` 已实现三级，`nacommand/mod.rs` 中 Config 分支返回了占位错误，需替换为实际调用。
3. **exec_script 临时文件**：创建在 `/tmp/nashell/` 下，文件名含随机串。执行后无论成功与否均删除。路径处理注意与配置文件所在目录的相对路径解析。
4. **外部命令支持 `known_modes`**：若外部命令定义了 `known_modes`，`build_nacommand` 会做查表提取 mode。对于大多外部命令此字段为空，mode 保持为 arg 透传。
5. **配置文件加载**：当前 `NashellConfig.na_commands` 已解析 KDL 的 `NaCommands` 块为 `HashMap<String, ExternalCmdConfig>`，只需在 `main.rs` 中将其转换为 `Vec<CmdMeta>` 写入 registry。

### 验证
- [x] 在 config.kdl 中配置 `websearch exec="python3 ./web_search.py" long_argument=false`
- [x] 调用 `!@WebSearch: rust async programming -n 5` → 正确执行并输出搜索结果
- [x] `!@WebSearch:Help` → 透传 `--help` 输出
- [x] 配置带 exec_script 的命令，临时脚本在 `/tmp/nashell/` 生成并自动清理
- [x] `exec` 字段支持空格分隔的程序名+参数（如 `"python3 ./script.py"`）
- [x] 306 个单元测试全部通过

### 额外完成项

- **`!@NaCmds:` 内置命令**（Phase 8 追加）：列出所有已注册 NaCommand 的 System 级命令
  - 默认模式：表格列出命令名、级别、来源（Builtin/Config/Plugin）
  - Detail 模式：额外显示每条命令的帮助摘要（内置直接获取、外部执行 `--help`、插件通过 call/response 协议）
  - `-j/--json` 选项：JSON 格式输出，ANSI 码自动清洗
  - `PluginManager::get_command_help()`：新增插件帮助获取方法，读取 stdout 后归还 handle
- **`build_nacommand` 加强**：`"help"` 成为所有命令的保留模式（含 `known_modes` 为空的外部/插件命令）
- **`config_dir` 路径解析**：`NashellConfig` 新增 `config_dir` 字段，`resolve_exec_parts` 按空格拆分 exec 并仅对 `./`/`../` 开头的路径做相对解析

---

### Phase 8 管道数据传递修复

**问题**：`!@NaCmds:Detail -j | from json` 报 "Pipeline empty"——管道中 NaCommand 的输出未能传递给下游 Shell 命令。

**根因**：
1. `exec_captured()` 没有 stdin 输入参数，管道中前一命令的输出（`pre_out`）在分派 Shell 命令时被丢弃
2. 更致命的是，nushell 的 `nu -c` 模式**不接收管道 stdin**（`printf 'data' | nu -c 'from json'` 中 `$in` 为空）

**修复方案**：

| 变更 | 文件 | 说明 |
|------|------|------|
| `exec_captured` 增加 `stdin_data` 参数 | `shell_exec.rs:150` | `None` 时保持原行为；`Some` 时按 shell_type 差异化注入 |
| bash 管道注入 | `shell_exec.rs:194` | `printf '%s' <data> \| bash -c '<cmd>'`（bash 的 `-c` 支持 stdin） |
| nu 临时文件注入 | `shell_exec.rs:166-192` | 写数据到 `/tmp/nashell/pipe_xxx` → `nu -c 'open <path> \| <cmd>'` |
| dispatch 传递 pre_out | `mod.rs:172` | `exec_captured(..., ctx.pre_out.as_deref())` |
| 纯 shell 管道优化 | `repl/mod.rs:366-409` | 全部为 `Shell` 类型时合并为单条 `shell -c 'cmd1 \| cmd2'` 执行 |
| NaCommand → long_argument | `repl/mod.rs:421-422` | 管道中 NaCommand 的 `pre_out` 自动成为 `long_argument` |

**管道语义**：

| 管道模式 | 行为 |
|----------|------|
| `Shell \| Shell` | 合并为单条 `shell -c`（原生管道） |
| `Shell \| NaCommand` | Shell 输出 → NaCommand 的 `long_argument` |
| `NaCommand \| Shell` | NaCommand 返回的 string → Shell 的 stdin（bash: printf pipe / nu: temp file） |
| `NaCommand \| NaCommand` | 前段输出 → `pre_out`（各 handler 自行处理） |

**新增测试**：4 个（`test_exec_captured_with_stdin_data`、`test_exec_captured_pipe_stdin_multiline`、`test_dispatch_shell_receives_pre_out_as_stdin`、`test_dispatch_shell_still_works_without_pre_out`），总测试数 310 通过。

---

## Phase 9: 错误处理、信号处理与退出 ✅ 已完成

**目标**：所有错误路径覆盖完毕，信号处理健壮，退出清理完整。

### 参考文档

| 内容 | 位置 |
|------|------|
| 错误处理 | `nashell_dev.md` → 错误处理 |
| 退出与信号处理 | `nashell_dev.md` → 退出与信号处理 |
| 输出截断 | `nashell_dev.md` → 输出截断策略 |
| 错误类型定义 | `INSTRUCTION.md` → 1.4 错误处理 |
| 错误显示格式 | `INSTRUCTION.md` → 六、实现注意事项 第7条 |

### 任务

1. ✅ **创建 `src/error/display.rs`**
   - `format_error(err: &NashellError) -> String`: 格式化错误为 `@Error #>>\n{类型}: {描述}` 格式
   - 不同错误类型的不同描述模板

2. ✅ **完善全局错误处理**
   - 审查所有 `?` 调用点，确保错误信息包含足够上下文
   - 所有 `match` 分支的 `None` 情况返回带上下文的错误
   - 配置文件缺失/损坏时优雅降级

3. ✅ **实现信号处理**
   - `SIGINT` (Ctrl+C)：中断当前执行中的命令，回到输入提示符
   - `SIGWINCH`：更新 PTY 窗口大小，保持提示符渲染正确
   - `SIGTERM` / `SIGHUP`：优雅退出
   - 连续两次 Ctrl+C：强制退出

4. ✅ **实现退出清理**
   - `exit` 命令和 Ctrl+D 处理
   - 清理顺序：插件 off → 异步 shell Destroy → main shell 关闭 → 临时文件清理 → 退出
   - 清理失败不阻塞退出（记录警告日志）

5. ✅ **输出截断**
   - `-c` 捕获模式下不截断
   - Open 命令按 `--limit` 截断

### 验证
- [x] 错误命令（如 `!@UnknownCmd:`）→ 显示明确的错误信息
- [x] 配置文件损坏 → 显示解析错误但程序正常启动
- [x] Ctrl+C 中断正在执行的 `sleep 30` → 回到提示符
- [x] 调整终端窗口大小 → 提示符和 PTY 输出正确响应
- [x] `exit` / Ctrl+D → 程序正常退出，无残留子进程
- [x] 检查退出后 `/tmp/nashell/` 无残留临时文件

330 个单元测试全部通过。

### 额外完成项

- **NaCommand 解析错误增强**：
  - **格式错误**（缺少冒号）：`!!@Bash ls` → 检测到缺 `:`，绿色 `Hint:` 提示正确格式
  - **级别错误**：`!@Bash:` → 识别 Bash 为 System 级命令，Hint 提示使用 `!!@` 前缀
  - **拼写模糊匹配**：`!@NaCmd:` → Levenshtein 编辑距离 ≤2 匹配到 `nacmds`，提示"你是不是想输入..."
  - **未知命令**：明确说出未知，提示 `!@NaCmds:` 查询
  - Hint 在 REPL 层通过注册表查表修正：格式错误提示中的前缀会根据命令注册级别自动纠正

- **Open → Read 重命名**：`!@Read:` 替代 `!@Open:`，语义更精确，与 nushell 内置 `open` 区分

- **插件 toExec 协议升级**：`to_exec` 从 `Vec<String>` 重构为 `ToExec` 结构体：
  ```json
  { "execs": ["cmd1", "cmd2"], "is_print": true, "timeout": 90 }
  ```
  - `is_print=true`：绿色提示 + 结果实时显示
  - `is_print=false`：灰色提示 + 纯捕获回传（`exec_captured`）
  - `timeout`：单条命令超时秒数（默认 90）

- **错误处理规范文档化**：`docs/plugin_dev.md` 新增第十二章，覆盖错误输出格式、NaCommand 解析错误、内置命令错误规范、插件错误报告

### Phase 9 复盘要点（Phase 10 开始前必须注意）

1. **NaFormatError 含注册表查表信息**：新增 `cmd_name` / `used_prefix` 字段，REPL 层通过 `enrich_error_with_registry()` 根据注册表修正 hint 前缀。Phase 10 添加新命令类型时注意此机制。

2. **信号处理使用 libc 信号处理器**：`src/repl/signals.rs` 安装 SIGINT/SIGTERM/SIGHUP 处理器，通过 `AtomicBool` 标志通信。SIGWINCH 被忽略（终端驱动自动处理）。双次 Ctrl+C 间隔 500ms 内判定为强制退出。

3. **退出清理函数 `cleanup()` 在 REPL 退出时调用**：顺序为插件 `stop_all()` → 异步 shell `destroy_shell()` → `/tmp/nashell/` 清理。任何步骤失败不阻塞后续清理。

4. **`levenshtein_distance` 函数**：在 `src/nacommand/registry.rs` 中，用于模糊匹配命令名。限制编辑距离 ≤ 2，返回最近匹配。后续可按需调整阈值。

5. **`fuzzy_suggest` 方法**：在所有已注册命令中检索，Phase 10 注册新命令时自动纳入模糊匹配范围。

6. **文件组织**：`src/repl/signals.rs`、`src/nacommand/builtin/read.rs` 遵循 INSTRUCTION.md 1.1 规范。已删旧的 `open.rs`/`open_tests.rs`。

---

## Phase 10: 集成测试与打磨

**目标**：端到端测试覆盖核心功能路径，边缘情况处理完善。

### 参考文档

| 内容 | 位置 |
|------|------|
| 全部功能定义 | `nashell_dev.md` 全文 |
| 全部实现规范 | `INSTRUCTION.md` 全文 |

### 任务

1. **单元测试**
   - 解析器：各种输入格式的解析正确性
   - NaCommand：Write/Open/Bash/Shell 各命令的正确性
   - 插件协议：消息序列化/反序列化
   - 配置加载：各种配置文件的解析

2. **集成测试**
   - 完整管道执行：`ls | !@Write:./out.txt @/` + 验证文件内容
   - 多行输入 + NaCommand：`!@Write:./test.py @/` + Python 代码
   - 异步执行：`echo hello @/Async(test)` + `!!@Shell:Watch` 验证
   - Shell Switch：创建异步 shell，切换到它，切回 main
   - 插件完整流程：注册 → call → response → toExec → off

3. **边缘情况**
   - 空输入（直接 Enter）
   - 超长输入（1000+ 行 long_argument）
   - 特殊字符在命令中（Unicode、emoji）
   - 嵌套引号（`"it's \"ok\""`）
   - 路径中有空格（`!@Open:"./my files/doc.txt"`）

4. **性能**
   - REPL 循环延迟应 < 50ms（不含命令执行时间）
   - PTY 输出透传延迟应 < 10ms
   - 大量输出时内存使用稳定（不发生泄漏）

5. **文档**
   - 代码中文档注释覆盖率 100%
   - README.md 包含安装和使用说明

### 验证
- `cargo test` 全部通过
- `cargo clippy` 无警告
- 所有 Phase 1~9 的验证项回归通过
