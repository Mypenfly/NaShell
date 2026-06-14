# NaShell

本项目脱胎于ncoding项目，旨在解决ncoding的旧命令系统（Command System）的语法奇怪，不易LLM/人类理解，在使用时限制了模型的能力/浪费了大量的token

ncoding项目的地址：https://github.com/Mypenfly/Ncoding.git

同时本项目是在Nedit项目的语法设计的启发和语义级的延伸，Nedit项目地址:https://github.com/Mypenfly/Nedit.git

同时在本项目的docs/目录下也有相关的介绍文档。

## 项目目标

打造一个基于现有shell,如nushell/bash的基础上的一个 "伪Shell"。
这个Shell的核心设计有：
1. 语义级的特殊命令，我们称为`NaCommand`
2. 特殊命令无缝兼容既有的shell命令和语法
3. 人与agent可以共用的Shell。（初步阶段不做内置agent系统，这一点只是表示人类和LLM的操作逻辑相同）
4. 支持多行/大量文本作为 `NaCommand`的输入

基本都架构是一个REPL循环（就是一个Shell的执行逻辑）注意：不是tui界面，支持外部命令执行和相关的彩色输出(ANSI彩色码保留)。

程序启动执行使用
```bash
  na
```

## 架构总览

采用**方案 A：单进程 + 多 PTY 会话**架构。

```
NaShell 主进程
├── REPL 前端 (rustyline / 自定义)
│   ├── 多行输入收集
│   ├── 提示符渲染 (ANSI 彩色)
│   └── 历史管理
├── 命令解析器
│   ├── 词法分析: 识别 !@ / !!@ / @/ / 管道 / 引号字符串
│   ├── 语法分析: 构建 RawCommands → Vec<RawCmd>
│   └── long_argument 提取 (优先 @/, 次选空行)
├── 命令分发与执行引擎
│   ├── 直连模式 (should_use_direct): Shell 命令 → Stdio::inherit 直连终端
│   │   └── cd 命令由 Rust 进程直接拦截 (std::env::set_current_dir)
│   ├── Captured 模式: 管道/异步/Bash → script -e -q -c 捕获执行
│   │   ├── 普通 Shell 命令 → exec_captured (shell -c wrapped by script PTY)
│   │   ├── !!@Bash: 命令 → exec_bash (bash -c wrapped by script PTY)
│   │   └── 管道编排: 逐段捕获, pre_out 传递
│   ├── NaCommand → 内置执行 / 外部程序 / 插件调用 (Phase 5+)
│   └── 安全拦截: check_safety() 在直连和 Captured 模式执行前检查 deny_patterns
├── Shell 管理器 (PTY)
│   ├── main PTY (portable-pty, 主工作环境)
│   ├── 异步 ShellActor (tokio task, 独立 PTY)
│   └── cwd 同步: PTY 天然保证
├── 插件管理器
│   ├── 子进程管理 (stdin/stdout NDJSON 帧协议)
│   ├── toExec 递归深度限制 (max_depth=3)
│   └── broadcast 广播通道
└── 配置加载器
    ├── ~/.config/nashell/config.kdl (KDL 解析)
    ├── ~/.config/nashell/plugins/ (manifest 扫描)
    └── 别名/Alias 解析
```

**PTY 持久化方案说明**：
- main Shell 是一个真正的、持久的 PTY 会话（使用 `portable-pty` ），其内部运行 `nu` 或 `bash`
- 这意味着 `cd` 等目录切换命令会真实地改变 shell 进程的工作目录，NaShell 通过轮询 `/proc/<pid>/cwd` 或监听 OSC 7 转义序列同步 cwd 到提示符显示
- 对于不需要捕获输出的纯 shell 命令，直接在 PTY 中交互执行，输出实时可见
- 对于需要捕获输出（管道中涉及 NaCommand）的场景，使用 `nu -c` / `bash -c` 单独执行并捕获结果

## 基础设定

1. 一般命令行命令（如"ls","mkdir","sudo"）通过 PTY 中的持久 shell 执行。程序启动时自动检测 `nu` 是否可用：
   - 若 `nu` 可用 → main PTY 运行 `nu`，所有 shell 命令由 nushell 执行
   - 若 `nu` 不可用 → main PTY 运行 `bash`，所有 shell 命令由 bash 执行
2. 当 main PTY 使用 `nu` 时，`!!@Bash:` 命令可用（用于临时回退到 bash -c 执行特定命令）。当 main PTY 使用 `bash` 时，`!!@Bash:` 仍可用但无实际回退意义（相当于直接 bash -c 执行）
3. 输出时在最开头（第一个有效输出内容之前）标识目前执行命令的 shell。只有 shell 命令执行时显示，NaCommand 执行输出使用 `@System #>>` 提示符
4. 命令行输入提示符设计示例如下：（提示符为绿色）
```text
  ~/projects/nashell |> ls -la
```
多行输入的提示符：
```text
  ~/projects/nashell |> !@Write:./example.py @/
                     >> x = int(input())
                     >> print(x * 18)
                     >> print("这是Nashell的多行输入的提示符示例，值得注意的是首行和其余行的输入提示符不同")
                     >> print("此处使用了命令`Write`,意味写入一个文件，后面行的内容就是写入的内容")
                     >> print("值得注意的是首行末尾使用的 `@/` 是命令/语句截至符号，标识着下面行的内容是作为命令Write的多行输入")
```
5. 命令输出也配置了提示符用于区分（提示符为灰色），示例如下：
```text
  @System #>>
  write ./example.py succeeded 
```
没有多行的提示符，只有首行的单独标识

6. 在命令输入中除了使用了 `NaCommand` 之外的语句/内容都作为 Shell 命令执行

7. 命令执行策略（双模式）：

   **直连模式**（单一命令、无管道、无异步、非 Bash）：
   - 子进程 stdin/stdout/stderr 全部继承 NaShell 进程的真实终端
   - 支持实时进度输出（curl、git clone）和交互输入（python REPL、read、TUI）
   - cd 由 Rust 进程通过 `std::env::set_current_dir()` 直接拦截处理
   
   **Captured 模式**（有管道 | / @/Async / !!@Bash:）：
   - 通过 `script -e -q -c "{shell} -c '{command}'" /dev/null` 在一次性 PTY 中执行
   - stdout/stderr 通过 pipe 捕获，用于管道传递或格式化输出
   - `script` 提供 TTY 环境 + `TERM=xterm-256color`，解决 TTY 检测问题
   - `Stdio::inherit()` 传真实终端 stdin，确保 script 能获取终端尺寸
   - 命令结束 script 自动退出，无哨兵/状态机

## 执行流（从输入到提交）

从用户按下 Enter 开始，完整的解析和执行流程，严格按以下优先级顺序：

### 阶段 0：输入收集

- 将多行输入（包括首行和所有 `>>` 行）收集为一个完整 String
- 去除首行开头的空格

### 阶段 1：`!!@Bash:` 优先检测

- 检查输入字符串的首个非空格词是否为 `!!@Bash:`
- 若是 → 将 `!!@Bash:` 之后、首个 `@/` 或首个空行之前的所有内容作为参数，直接调用 `bash -c "{args}"` 执行，实时输出。此路径直接结束，跳过后续所有阶段
- 注意：此阶段检测成功后，"之后的管道也作为参数写入"，不执行管道分割
- 若 `!!@Bash:` 行尾有 `@/Async(name)` → 创建异步 bash 子进程执行，命令结束后进程自动退出，不持久化

### 阶段 2：`@/Async` 异步标记检测

- 检查首行末尾是否有 `@/Async(name)` 模式（`@/` 后紧跟 `Async(...)`）
- 若有 → 记录 `async_name = name`，并将 `@/Async(name)` 视为截止符（同 `@/`）
- 若无 → 继续到阶段 3

### 阶段 3：long_argument 提取

**优先级规则：`@/` 优先于空行。** 一旦通过某规则提取成功，不再尝试后续规则。

- **规则 A（优先）**：查找首个 `@/` 截止符
  - 将 `@/` 之前的内容收集为一个 Vec（后续按管道分割）
  - 将 `@/` 之后的行内容收集为 `long_argument: String`（原样保留格式）
  - `@/` 之后的 `>>` 提示符行去掉 `>> ` 前缀后拼入 long_argument
- **规则 B（回退）**：若无 `@/`，查找首个空行作为分割
  - 将首个空行之前的行作为命令语句收集
  - 将首个空行之后的行内容收集为 `long_argument: String`
- **规则 C（两项皆无）**：整个输入没有 `@/` 也没有空行
  - long_argument 为 `None`，整个输入进入命令解析

### 阶段 4：管道分割

- 对阶段 3 提取的命令语句部分（`@/` 或空行之前的内容），按管道符号 `|` 分割为多个命令段
- 分割时保护引号内的 `|`（`"a|b"`, `'a|b'` 中的 `|` 不分割）
- 若没有 `|` 则整个为一个命令段

### 阶段 5：逐个命令段解析与执行

对每个命令段（按管道顺序），提取首个词/命令 (index=0) 并判断类型：

**类型 A：`!!@Bash:`**
- 将整个命令段（包括后续内容）作为 bash -c 的参数执行，捕获输出
- 不接受管道传递的 pre_out（因为 Bash 命令本身就是一个完整的 bash 语句）

**类型 B：`!!@NaCommand:` 或 `!@NaCommand:`（NaCommand）**
- 提取命令名 `NaCommand` 和可选的子命令/模式（`NaCommand:Mode` 格式）
- 解析命令行参数（`-q "..." -c 10` 等选项参数）
- 在 `AppData` 中查表：先查内置命令 → 再查用户配置的 external 命令 → 再查插件注册的命令
- 如果该命令是管道第一个命令，则 long_argument 使用阶段 3 提取的内容
- 如果该命令在管道中间/末端，则 pre_out（前一个命令的捕获输出）作为该 NaCommand 的输入
- 执行并捕获输出，传递给下一个管道段或最终打印
- 若查表失败（命令未注册），报错 `未知的 NaCommand: xxx`

**类型 C：普通 Shell 命令段**
- 若整个管道链中没有任何 NaCommand → 直接在 main PTY 中交互执行，实时输出
- 若管道链中有 NaCommand → 通过 `nu -c` / `bash -c` 静默捕获输出，传递给下一个命令段

### 阶段 6：管道输出传递

- 每个命令段执行完毕后，若后面还有命令段（有管道），将输出通过管道传给下一个命令段
- 最后一个命令段的输出打印到终端（如果是 shell 管道段则保留 ANSI 彩色码，如果是 NaCommand 输出则格式如 `@System #>>\n{output}`）

### 阶段 7：循环结束

- 所有输出打印完毕，如果阶段 2 检测到 `@/Async(name)`：
  - 若已有同名 shell → 使用已有 shell 线程异步执行
  - 若无同名 shell → 创建新 PTY 线程 + 分配随机 id，异步执行
  - 执行结果存入 pools，返回提示 `shell created and exec: {command}\nname: {name}  id: {id}`
- 重新显示输入提示符，等待下一次输入

---

参考的解析后数据结构：
```rust
  ///收集的命令，基本通过这个的各种方法来执行
  struct RawCommands {
    ///解析后的命令集合
    commands:Vec<RawCmd>,
    ///"@/"后的内容，收集为一个大参数
    long_argument:Option<String>,
    ///捕获的前一个命令执行的结果，用于管道
    pre_out: Option<String>,
    ///是否异步执行
    is_async: Option<String>, // 存储 Async(name) 中的 name
  }
  ///每个命令的解析
  struct RawCmd {
    ///表明类型方便执行
    type_: CmdType,
    ///命令，通常是执行程序，如 ls, vim 或者没有特殊格式的NaCommand 如 Write
    cmd: String,
    ///参数
    args:Vec<String>,
  }
   ///命令类型枚举
   enum CmdType {
     ///一般的Shell命令没有任何特殊标识
     Shell,
     ///普通的NaCommand,!@NaCommand
     NaCommandNormal,
     ///系统级的NaCommand,!!@NaCommand
     NaCommandSystem
   }
```

## Shell 命令执行（直连 / Captured 双模式）

### 直连模式（`exec_shell_direct`）

条件：整条输入 = 单一命令 ∧ 无管道 `|` ∧ 无 `@/Async` ∧ 非 `!!@Bash:`

```
Rust Command → {shell} -c '{command}'  (stdio 全部 inherit)
```

- stdin/stdout/stderr 全部继承自父进程，子进程直接读写真实终端
- 适用于 **实时输出**（curl 进度、git clone 百分比）和 **交互式输入**（python REPL、read、TUI）
- `cd` 命令由 Rust 进程拦截，不经过此路径

### Captured 模式（`exec_captured`）

条件：有管道 `|` / 有 `@/Async` / `!!@Bash:`

```
Rust Command → script -e -q -c "{shell} -c '{command}'" /dev/null
```

- `script` 分配伪终端，命令感知到 TTY 后正常输出彩色
- stdout/stderr 通过 pipe 捕获为 String，保留 ANSI 彩色码
- 捕获的输出用于管道传递（pre_out → 下一命令段）或格式化打印
- 退出码通过 `script -e` 正确传递
- 支持超时机制：超时后 SIGTERM → SIGKILL 终止子进程

### Bash 命令（`exec_bash`）

`!!@Bash:` 专用路径——直接构造 `bash -c '{args}'` 不经过双层 shell 包装。
输出使用亮黄色 `Bash:` 标识（来自 `bash_output_prompt_fg` 配置）。

### 管道执行

管道中存在 NaCommand 时，逐段通过 Captured 模式执行，前段输出（pre_out）传入后段。
纯 shell 管道（不含 NaCommand）可整体交给 shell 原生管道处理（优化，尚未实现）。

### Shell 管理数据结构

```rust
  struct Shell {
    name : String,
    id : String,
    ///工作路径，通过 PTY 轮询 /proc/{pid}/cwd 或 OSC 7 序列同步
    path: PathBuf,
    ///PTY 子进程 handle
    pty: PtyHandle,
    receiver: Receiver<ShellCmd>,
    sender: Sender<ShellOut>,
    ///用于积累输出，一次命令为一条，用于 name != "main" 的异步 shell
    pools: Vec<String>
  }
  ///执行的命令枚举
  enum ShellCmd {
    ///在 PTY 中直接执行 (将命令文本写入 PTY stdin)
    ExecPty { input: String },
    ///通过 -c 执行并捕获输出
    ExecCaptured { cmd: String, args: Vec<String> },
    ///重命名/将main shell快速切换到另一环境中
    Switch(String),
    ///中断命令执行
    Stop,
    ///销毁
    Destroy,
    ///获取pools中的内容，根据数量
    Watch { count: usize },
    ///获取这个Shell的状态，包括name,path,id,pools的数量
    GetState,
  }
  ///Shell内容传出
  enum ShellOut {
    ///实时输出 (PTY 模式下逐块透传)
    Working(String),
    ///捕获输出结束 (用于 -c 模式)
    Captured { stdout: String, stderr: String, exit_code: i32 },
    ///等待, 命令执行完毕
    Wait,
    ///被销毁的确认反馈
    Destroyed,
    ///重命名的反馈
    Switched { new_name: String, id: String },
    ///状态的传出
    State {
      name: String,
      id: String,
      path: String,
      pools_count: usize,
    },
  }
```

> 对于 main Shell 说是异步的，但实际上它的执行会阻塞主线程，这是设计不是缺陷，之所以设计成异步是为了 Switch 逻辑处理方便以及持久的、独立的 Shell 环境。

## Alias 命令别名

alias 配置允许用户为常用命令定义简写。展开逻辑为简单的首词替换（首词匹配则替换为 alias 值，保留后续参数）。

**alias 配置格式**（在 config.kdl 中）：
```kdl
alias {
    ll "ls -la"
    gst "git status"
    // alias 的值直接作为命令文本替换
}
```

> alias 展开在解析之前执行，对所有类型的命令生效

## `NaCommand`

本项目最关键的部分，也是一大特色。

### NaCommand 的分级机制

分两级，一个为 Normal 级（`!@NaCommand:`），一个为 System 级(`!!@NaCommand:`)

两级划分标准：
1. System 级：影响 NaShell 本身（如维护 Shell 数量、切换执行环境、操作内部状态）
2. Normal 级：不影响 NaShell 内核，仅在当前用户工作环境中产生作用

初步阶段大部分命令都是 Normal 级别，只有等到集成 agent 才会有更多 System 命令。

统一数据结构：
```rust
///NaCommand 数据结构
struct NaCommand {
  ///等级区分
  level: NaLevel,
  ///具体命令
  cmd: String,
  ///子命令/模式，如 Shell:Watch 此时 watch 就是 mode
  mode: Option<String>,
  ///命令行参数，如 " -q ... -c ..."
  args: Vec<String>,
  ///多行长参数(来自用户写入，或者通过管道获取的pre_out)
  long_argument: Option<String>,
}
///命令级别后面根据这个来执行不同的逻辑
enum NaLevel {
  Normal,
  System
}
```

### long_argument 的提取方式

long_argument 是本项目解决 LLM 场景下内容转义问题的核心机制。

**提取优先级（已在前文阶段 3 中详述）：**
1. **优先使用 `@/` 截止符**：提取第一个 `@/` 之后的所有行内容为 long_argument
2. **回退使用空行分割**：若无 `@/`，则以首个空行作为命令语句与内容的边界

两种方式不能同时生效——一旦 `@/` 存在，空行被视为 long_argument 的一部分，不再分割。

对于大部分命令（尤其是用户配置的外部命令/插件命令），一般只接受一个 long_argument，需要特别指明对应的 long_argument 参数名。

### System 级命令

相较于 Normal 命令，系统命令总是系统内部集成的既有命令，所以有特殊的性质和执行逻辑。
**所有 System 级命令都不接受 long_argument 参数**（即使用阶段 3 提取的 long_argument 始终为 None 传给 System 命令）。

#### Bash

使用示例:
```text
  ~/projects/nashell |> !!@Bash: ls -la 
```
这个命令的目的是为了解决部分无法/不便用 nushell 解决时临时回退到 bash -c 执行。

执行逻辑：
1. Bash 命令的检测优先级最高（见执行流阶段 1），一旦识别为 `!!@Bash:`，立即短路后续所有解析
2. `!!@Bash:` 之后到行末/`@/`/空行之前的所有内容作为 bash -c 的参数
3. 将参数填充到 `bash -c "{args}"` 中执行，实时输出结果
4. 输出结果开头用亮黄色标识 `Bash:`
5. 若命令末尾有 `@/Async(name)` → 创建异步 bash 子进程执行，命令结束进程自动退出（不持久化为 Shell 线程）

> 注意：`!!@Bash:` 的优先级覆盖所有其他解析规则，包括管道 `|`、`@/`（除 `Async` 外）、NaCommand 等

#### Shell

Shell 命令用于管理 NaShell 内部的 Shell 线程。接受以下模式：

**1. 默认模式（无 mode）**：获取所有 Shell 的状态

```
  ~/projects/nashell |> !!@Shell:
   
  @System #>>
    Shells states
    name    id      path                  pools_count
    main    2e4e..  /home/user/projects   0
    test    a1b2..  /tmp                  3
```

**2. Watch 模式**：查看指定 shell 的 pools 输出

接受命令行参数：
- `--id/-i [string]` 必传
- `--count/-c [int]` 可选，默认 1（从后往前取最后 N 个）

```text
  ~/projects/nashell |> !!@Shell:Watch -i "2e3.." -c 3

  @System #>>
    shell pools
    name: test    id: 2e3..
    pool index (2):
      ...(输出内容)...
    pool index (1):
      ...(输出内容)...
    pool index (0):
      ...(输出内容)...
```

**3. Destroy 模式**：通过 id 删除对应的 shell 线程

接受命令行参数：
- `--id/-i [string]` 必传

```text
  ~/projects/nashell |> !!@Shell:Destroy -i "2e3.."

  @System #>>
    shell has been destroyed
    name: test    id: 2e3..
```

**4. Switch 模式**：通过 id 将目前的 main shell 切换为对应 id 的 shell

接受命令行参数：
- `--id/-i [string]` 必传
- `--destroy/-d` 可选，若选择则销毁旧的 main Shell

```text
  ~/projects/nashell |> !!@Shell:Switch -i "2e3.." -d

  @System #>>
    main shell has been switched from main(2e4e..) to test(2e3..)
    old shell destroyed
    name: main    id: 2e4e..
```

> 实现逻辑：通过 `ShellCmd` 的线程间通信实现，系统通过 `name == "main"` 来识别主 shell（而非 id），切换时修改目标 shell 的 name 为 "main"

### `@/Async(name)` 异步 Shell 执行

通过截止符 `@/Async(name)` 实现，工作流程：
1. 提取 `Async(name)` 中的 name
2. 若已有同名 shell → 复用该 shell 线程
3. 若无同名 shell → 创建新 PTY shell 线程，分配随机 id
4. 将截止符之前的命令语句给到这个异步 shell 解析（流程和主线程一致，但运行在独立 PTY 中）
5. 异步 shell 执行结果写入其 pools 中
6. 线程创建后立即返回提示：`shell created and exec: {command}\nname: {name}  id: {id}`

> 注意：这里的 shell 创建是指在新的 PTY 会话中运行整个 NaShell 的命令解析和执行逻辑（类似于 fork 了程序的命令执行能力），而不仅仅是开一个 `nu -c` 进程

### Normal 级命令

一般的命令，相当于一般 shell 中的 alias 加上 long_argument 的特性。

#### Write

用于写入文件内容。

参数:
- `path` 路径，必须（命令名后的第一个参数）
- `content` 要写入的内容，是 long_argument，非必须（没有就是创建新文件 / 清空既有文件内容）

```text
  ~/projects/nashell |> !@Write:./example.py @/
                     >> x = int(input())
                     >> print(x * 18)
```

执行逻辑：
1. 提取 `path`（`./example.py`）
2. 提取 `content`（long_argument，`@/` 之后的内容）
3. 检查父目录是否存在，若不存在则报错并给出提示
4. 写入文件（创建或覆盖）
5. 返回格式：
```text
  @System #>>
    write to /absolute/path/to/example.py (256 bytes)
```

#### Open

用于打开文件/文件夹。

参数:
- `path` 路径，必须
- `--limit/-l [int]` 限制行数，非必须，默认 500 行
- `--start/-s [int]` 起始行，非必须，默认第 1 行
- `--end/-e [int]` 结束行，非必须，默认从起始行到起始+限制行数之间

> `--limit`、`--start`、`--end` 仅在 `path` 为文件时可用，若 path 为目录时传入了这些选项则报错

使用示例（目录）：
```text
  ~/projects/nashell |> !@Open: ./

  @System #>>
    (输出目标目录下的结构树，类似 eza -T 或 ls -laR)
```

使用示例（文件）：
```text
  ~/projects/nashell |> !@Open: ./test.py -l 200

  @System #>>
    1  import os
    2  import sys
    3  
    4  def main():
    5      print("hello")
    ...
```
> 文件内容输出格式：`行号  内容`。考虑语法高亮，参考 bat 的实现逻辑

#### Help 模式

无论是 System/Normal 内置命令、用户自行配置的命令，还是插件提供的命令，都统一支持一个 help 模式。

使用方式：`NaCommand:Help`

输出格式建议：
```
  {{NaCommand 的名字}}
  {{简要的功能介绍}}
  {{分模式介绍不同的参数选项，尤其是其中的 long_argument}}
  {{分模式的使用示例}}
```

对于内置命令 → 输出内置的帮助信息
对于用户配置的外部命令 → 映射为对外部程序传入 `--help` 参数，直接透传其输出
对于插件提供的命令 → 对插件发送 call 消息（mode 为 help），由插件返回帮助信息

#### 用户外部命令自行配置

用户可以在配置文件中自行配置一些 Normal 命令。

配置示例（config.kdl 的 NaCommands 块）：
```kdl
NaCommands {
    // 一个编辑用的工具配置，支持 long_argument，通过脚本执行
    edit exec="n_edit" long_argument=true exec_script=".ned"
    // 一个联网搜索工具，不支持 long_argument，也不支持脚本执行
    websearch exec="nu ./web_search.nu" long_argument=false
    // 使用的相对路径是相对于配置文件的相对路径，而非程序工作路径
}
```

> 命令名称匹配是大小写不敏感的，所有命令名和模式名按小写匹配。`WebSearch`、`websearch`、`WEBSEARCH` 均匹配为 `websearch`

对于支持 `exec_script` 的命令（如 edit），执行逻辑：
1. 读取 long_argument
2. 将 long_argument 保存为临时脚本文件，后缀使用指定的后缀（如 `.ned`）
3. 以临时脚本路径作为 `exec` 程序的参数执行，形如：`n_edit /path/to/tmp/script.ned`
4. 执行结束后清除临时脚本文件

> 若不通过脚本执行（`exec_script` 为空），则 long_argument 作为字符串直接传给 exec 程序的最后一个参数

Help 模式对外部命令的处理：直接传入 `--help`，透传其输出。

对于外部命令的子模式，映射规则为：
- `!@WebSearch:Today -c 5` → `nu /path/to/web_search.nu today -c 5`
- 即 mode 短语直接作为 exec 程序的第一个参数

## 插件系统

NaShell 是一个框架，通过插件实现功能扩展。

### 通信协议

插件通过 **NDJSON**（Newline Delimited JSON）格式与主程序进行通信。每条消息是一个完整的 JSON 对象，以换行符 `\n` 分隔。

> 注意：这不是真正的"流式"通信（如 SSE），而是一行一条消息的帧协议。每收到一行完整的 JSON 即可解析处理。

基本消息格式：
```json
{
  "sender": "nashell",
  "type": "call",
  "data": {}
}
```

字段说明：
- `sender`：发起方标识，插件发送时填插件名，主程序发送时填 `"nashell"`
- `type`：消息类型，支持 `call`、`broadcast`、`response`、`off`
- `data`：消息载荷，随 type 不同而变化

### 消息类型

#### 1. call（主程序 → 插件）

主程序在匹配到插件注册的命令时发送。

```json
{
  "sender": "nashell",
  "type": "call",
  "data": {
    "command": "agent",
    "mode": "setting",
    "level": "system",
    "params": ["--model", "deepseek-v4-pro", "--provider", "deepseek"],
    "long_argument": null
  }
}
```

- `command` 和 `mode` 已小写化
- 若未指定 mode，则 mode 字段为 `"normal"`（代表命令的默认执行模式）
- `long_argument` 为 null 时表示无长参数

#### 2. response（插件 → 主程序，流式输出）

插件在执行过程中发送的中间结果或流式输出。

```json
{
  "sender": "agent",
  "type": "response",
  "data": {
    "streaming": true,
    "out_content": "这里是输出的内容，可包含 ANSI 彩色码",
    "out_prompt": "@agent #>>",
    "is_print": true,
    "to_exec": [],
    "exec_result": null
  }
}
```

字段说明：
| 字段 | 类型 | 说明 |
|------|------|------|
| `streaming` | bool | 为 true 表示后续还有 response 消息 |
| `out_content` | string | 输出的内容，应包含所需的格式（ANSI 彩色码等） |
| `out_prompt` | string/null | 输出提示符，若 is_print 为 true 且此字段有值，则在 out_content 前打印此提示符 |
| `is_print` | bool | 是否实时打印 out_content 到终端 |
| `to_exec` | [string] | 要求主程序代为执行的命令列表 |
| `exec_result` | [string]/null | to_exec 的执行结果（由主程序填充后发回） |

**toExec 执行流程**：
1. 主程序收到含有 `to_exec` 的 response 消息后，暂停该插件的输出处理
2. 按 `to_exec` 列表顺序逐条执行命令（流程与用户输入完全一致）
3. 收集每条命令的最终输出，填入 `exec_result` 数组（顺序一一对应）
4. 主程序发送新的 response 消息（含 `exec_result`）给插件
5. 插件继续执行

**toExec 递归限制**：为了防止无限递归，设置最大递归深度 `max_depth = 3`：
- 深度 0：用户直接输入触发的命令
- 深度 1：插件 toExec 中触发的命令
- 深度 2：上述命令触发的插件 toExec
- 深度 3：最后一层允许的 toExec
- 深度 > 3：不再解析 toExec 中的插件命令，仅当做纯 shell 命令执行（NaCommand 直接报错）

#### 3. off（插件 → 主程序，结束通知）

插件执行完毕时发送。

```json
{
  "sender": "agent",
  "type": "off",
  "data": {
    "to_exec": [],
    "out_content": "...",
    "out_prompt": "...",
    "is_print": true
  }
}
```

主程序收到 off 后：
1. 若 `to_exec` 非空 → 执行 toExec 流程（但不再发回结果，因为插件即将关闭）
2. 若 `is_print` 为 true → 打印 `out_content`
3. 关闭插件进程

#### 4. broadcast（主程序 ↔ 插件，广播）

主程序广播消息给所有注册了 `is_broadcast: true` 的插件。

```json
{
  "sender": "nashell",
  "type": "broadcast",
  "data": {
    "event": "shell_state_changed",
    "payload": { "name": "main", "path": "/home/user/new_dir" }
  }
}
```

- 主程序在特定事件发生时广播（如 shell 切换、cwd 变更）
- 注册了 broadcast 的插件收到消息后可自主处理
- 无回复要求

### 插件配置与注册

所有插件统一管理于 `~/.config/nashell/plugins/` 下，按不同插件分文件夹管理。

启动时读取 `~/.config/nashell/plugins/{plugin_name}/manifest.json`：

```json
{
  "name": "the plugin name",
  "exec": "/path/to/the/plugin/exec",
  "nacommands": {
    "agent": {
      "level": "system",
      "long_argument": true
    },
    "context": {
      "level": "system",
      "long_argument": false
    },
    "session": {
      "level": "normal",
      "long_argument": false
    },
    "config": {
      "level": "system",
      "long_argument": true,
      "exec_script": ".conf"
    }
  },
  "is_broadcast": false
}
```

字段说明：
- `name`：插件名称
- `exec`：插件可执行文件路径
- `nacommands`：注册的 NaCommand，key 为命令名，value 包含 level、long_argument 支持、可选的 exec_script
- `is_broadcast`：是否订阅主程序的 broadcast 消息

> 启动时立即启动并保活所有 `is_broadcast: true` 的插件

## 完整执行流

以下是从程序启动到一次用户交互结束再到等待下一次输入的完整流程。

### 初始化阶段

```rust
///程序的主要可查询的数据
struct AppData {
  builtin_cmds: Vec<Cmd>, // 内置命令，如 Bash、Shell、Write、Open 等
  config_cmds: Vec<Cmd>,  // 用户配置的 external NaCommand
  plugins: Vec<Plugin>,   // 插件注册表
  main_shell: Shell,      // 主 PTY Shell
  async_shells: HashMap<String, Shell>, // name → Shell
}
///命令元数据
struct Cmd {
  level: Level,
  name: String,
  exec: String,
  long_argument: bool,
  exec_script: Option<String>,
}
///插件元数据
struct Plugin {
  name: String,
  exec: String,
  is_broadcast: bool,
  commands: Vec<Cmd>,
  process: Option<ChildProcess>, // 运行时进程 handle
}
enum Level { Normal, System }
```

初始化步骤：
1. 程序启动
2. 读取配置文件 `~/.config/nashell/config.kdl`
3. 加载 NaCommands 配置到 `config_cmds`
4. 加载 Alias 配置
5. 遍历 `~/.config/nashell/plugins/` 目录，解析每个 manifest.json，注册到 `plugins`
6. 启动所有 `is_broadcast: true` 的插件并保活
7. 检测 `nu` 可用性，确定 main PTY 的 shell 类型
8. 启动 main PTY Shell 线程
9. 显示 opening 内容（按配置的 fastfetch / 横幅文件 / 默认横幅）
10. 进入 REPL 循环，显示输入提示符

### 用户交互循环（每次 Enter 提交）

1. 读取用户多行输入，收集为完整 String
2. 按**执行流阶段 1~7**（见前文）依次解析并执行
3. 收集最终输出，打印到终端
4. 所有异步任务已启动（如有 `@/Async`）
5. 重新显示输入提示符，回到步骤 1

### 数据流转关系

用户输入 String
  → [解析器] → RawCommands { commands: Vec<RawCmd>, long_argument, pre_out, is_async }
    → [分派器] 遍历 RawCmd:
      - CmdType::Shell → ShellCmd::ExecPty / ShellCmd::ExecCaptured → ShellActor (PTY)
      - CmdType::NaCommandNormal / NaCommandSystem → 查 AppData → 执行
        → 内置: 直接调用对应 handler
        → 外部配置: 调用 exec 程序
        → 插件: call 消息 → 插件进程 → response/off 消息
    → 管道连接: 每个命令段输出 → 下一命令段输入
    → 最终输出 → 打印到终端

## 配置文件完整 Schema

配置文件路径：`~/.config/nashell/config.kdl`

```kdl
// ===== 程序启动显示 =====
opening {
    // 方式一：执行命令（如 fastfetch）
    exec "fastfetch"
    // 方式二：显示文件中的终端艺术画
    // file "/path/to/banner.txt"
    // 若不指定，则显示 NaShell 默认横幅
}

// ===== 提示符样式 =====
prompts {
    // 输入提示符（绿色）
    input_prompt_fg "green"
    // 输入提示符格式：{path} |> 
    input_prompt_format "{path} |> "
    // 多行输入提示符
    input_continue_format ">> "
    // 输出提示符（灰色），用于 NaCommand 的 @System #>>
    output_prompt_format "@System #>>"
    output_prompt_fg "gray"
    // Shell 输出提示符（亮黄色），用于 !!@Bash: 的输出
    bash_output_prompt_fg "bright_yellow"
    // Shell 类型标识颜色
    shell_type_fg "blue"
}

// ===== NaCommand 外部命令配置 =====
NaCommands {
    // 格式：<命令名> exec="<可执行文件>" long_argument=<true|false> [exec_script="<后缀>"]
    // exec 的相对路径相对于本配置文件所在目录
    edit exec="n_edit" long_argument=true exec_script=".ned"
    websearch exec="nu ./web_search.nu" long_argument=false
}

// ===== 命令别名 =====
alias {
    // 格式：<别名> "<展开后的命令>"
    // 普通别名
    ll "ls -la"
    gst "git status"
}

// ===== Shell 设置 =====
shell {
    // 默认超时（秒），仅对 -c 捕获模式有效
    timeout_secs 120
}

// ===== 安全设置 =====
safety {
    // 禁止执行的命令模式（正则匹配）
    deny_patterns [
        "sudo ",
        "rm -rf /",
        "rm -rf /*",
        "chmod 777 /",
        "dd if=",
        "> /dev/sda"
    ]
}

// ===== 插件设置 =====
plugins {
    // 插件目录
    dir "~/.config/nashell/plugins"
    // toExec 最大递归深度
    max_recursion_depth 3
}
```

### 配置加载优先级

1. 环境变量 `NASHELL_CONFIG` 指定的配置文件（最高优先级）
2. `~/.config/nashell/config.kdl`（默认）
3. 程序内置默认值（最低优先级，当上述文件不存在且配置项缺失时使用）

## 错误处理

### 解析错误

| 错误场景 | 处理方式 |
|----------|---------|
| 引号未闭合（如 `"hello world`） | 报错并提示：缺少闭合引号 |
| NaCommand 未知（查表失败） | 报错：`未知的 NaCommand: {name}` |
| NaCommand 缺少必要参数 | 报错并显示该命令的 help 信息 |
| 配置文件解析失败 | 显示错误位置，使用默认配置继续启动 |

### 执行错误

| 错误场景 | 处理方式 |
|----------|---------|
| shell 命令返回非 0 退出码 | 正常显示退出码和 stderr，不中止后续命令 |
| PTY 子进程异常退出 | 显示错误信息，尝试重启 main shell |
| 插件进程崩溃 | 显示错误信息，标记该插件不可用 |
| 插件通信超时（如 30 秒无响应） | 强制关闭插件进程，报错 |
| 命令执行超时（超时秒） | 终止执行，显示超时信息 |
| 文件操作失败（Write/Open） | 显示具体错误原因（路径不存在/权限不足） |

### 安全拦截

| 触发条件 | 处理方式 |
|----------|---------|
| 匹配 deny_patterns 中的模式 | 拒绝执行并显示：`此命令已被安全策略拦截` |
| `rm` 目标在工作目录外 | 拒绝执行并显示：`rm 目标不在当前工作目录内，请手动执行` |

### 所有错误信息格式

```
@Error #>>
{错误类型}: {具体描述}
```

## 退出与信号处理

### 退出机制

- **`exit` 命令**：在输入中直接输入 `exit`（不含任何特殊标记），正常退出 NaShell 并清理所有子进程和插件
- **`Ctrl+D` (EOF)**：在空输入时按 Ctrl+D，效果同 exit
- **`Ctrl+C` (SIGINT)**：
  - 若有命令正在执行：中断当前命令（发 SIGINT 到 PTY 子进程），但不退出 NaShell
  - 若在空输入状态：效果同 exit，退出 NaShell
  - 连续两次 Ctrl+C：强制退出（不等待清理）

### 信号处理

| 信号 | 行为 |
|------|------|
| SIGINT (Ctrl+C) | 中断当前执行中的命令，回到输入提示符 |
| SIGTERM | 优雅退出：清理所有子进程和插件，释放 PTY |
| SIGWINCH | 终端窗口大小变化：更新 PTY 的窗口大小，保持与终端同步 |
| SIGHUP | 同 SIGTERM，优雅退出 |

### 退出清理顺序

1. 发送 off 消息给所有活跃插件，等待它们关闭
2. 销毁所有异步 shell 线程（发送 Destroy 命令）
3. 关闭 main PTY shell
4. 清理临时文件（如 exec_script 产生的临时脚本）
5. 退出进程

## 输出截断策略

对于过长的输出（来自 `-c` 捕获），默认不截断（完整传递）。但在以下情况截断：

| 场景 | 截断规则 |
|------|---------|
| pipeline 内部传递 | 不截断，完整传递以保证数据完整性 |
| `!!@Shell:Watch` 输出 | 不截断 |
| `!@Open:` 文件读取 | 按 `--limit` 参数限制行数，默认 500 行 |
| 插件 response/off 的 out_content | 由插件自行控制，主程序不做截断 |

## 未来展望内容

1. 丰富提示符的可用样式，可以的话去兼容/使用 starship 的样式配置方式
2. 目前插件的可接入点只有命令识别/执行处，希望扩展更多可接入点
3. 集成 agent 系统，实现人类与 agent 共用 Shell
