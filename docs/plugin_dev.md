# NaShell 插件开发指南

本文档详细描述 NaShell 插件系统的通信协议、消息格式、生命周期和开发规范，
旨在帮助开发者编写自定义插件以扩展 NaShell 的功能。

---

## 一、插件系统概述

NaShell 插件是独立的可执行程序（任何语言），通过 **stdin/stdout** 与主程序进行
**NDJSON**（Newline Delimited JSON）帧协议通信。

### 核心特性

| 特性 | 说明 |
|------|------|
| **语言无关** | 任何能读写 stdin/stdout 并处理 JSON 的语言均可 |
| **命令注册** | 插件通过 manifest.json 声明提供的 NaCommand |
| **流式输出** | 支持多段实时响应（逐条打印，无需等待全部完成） |
| **toExec 委托** | 插件可请求主程序代为执行 shell / NaCommand 命令 |
| **广播事件** | 插件可订阅系统事件（shell 切换、cwd 变更等） |
| **安全隔离** | 插件作为独立子进程运行，崩溃不影响主程序 |

### 架构

```
NaShell 主进程
  │
  ├─ 插件子进程 × N
  │   ├─ stdin  ← NaShell 发送 call / response / broadcast
  │   └─ stdout → 插件发送 response / off
  │
  └─ Broadcast（主程序 → 所有 is_broadcast 插件）
```

---

## 二、目录结构与清单文件

### 插件目录

所有插件放置在 `~/.config/nashell/plugins/` 下，每个插件一个子目录：

```
~/.config/nashell/plugins/
├── my_plugin/
│   ├── manifest.json    ← 插件清单（必须）
│   ├── plugin.py        ← 插件主程序
│   └── ...              ← 其他依赖文件
└── another_plugin/
    ├── manifest.json
    └── ...
```

### manifest.json 格式

```json
{
    "name": "my_plugin",
    "exec": "python3 /absolute/path/to/plugin.py",
    "nacommands": {
        "command_name": {
            "level": "normal | system",
            "long_argument": true | false,
            "exec_script": ".ext" 
        }
    },
    "is_broadcast": false
}
```

| 字段 | 类型 | 必须 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 插件唯一名称，用于进程管理和日志 |
| `exec` | string | 是 | 可执行文件路径。支持带参数形式，如 `"python3 /path/to/plugin.py"`。**建议使用绝对路径** |
| `nacommands` | object | 否 | 注册的 NaCommand，key 为命令名，value 为命令配置 |
| `nacommands.<name>.level` | string | 是 | `"normal"`（普通级）或 `"system"`（系统级） |
| `nacommands.<name>.long_argument` | bool | 是 | 是否接受 `@/` 后的多行长参数 |
| `nacommands.<name>.exec_script` | string | 否 | 临时脚本后缀。如 `".py"` 则 long_argument 保存为临时 `.py` 文件，路径作为 exec 参数 |
| `is_broadcast` | bool | 否 | 是否订阅主程序广播事件。默认 `false` |

### 命令级别说明

- **Normal**（`!@Cmd:`）：普通命令，在用户工作环境中产生作用
- **System**（`!!@Cmd:`）：系统级命令，影响 NaShell 自身（如管理 shell 线程）

---

## 三、通信协议

### 传输层

- **通道**：stdin（NaShell → 插件）、stdout（插件 → NaShell）
- **编码**：UTF-8
- **格式**：NDJSON — 每条消息为一行完整 JSON，以 `\n` 结尾
- **规则**：
  - 空行会被自动跳过
  - JSON 内部不得包含未转义的换行符
  - 每条消息写入后必须 `flush`

### 消息信封

所有消息共享外层结构：

```json
{
    "type": "call | response | off | broadcast",
    "sender": "发送方标识",
    "data": { }
}
```

| 字段 | 说明 |
|------|------|
| `type` | 消息类型，决定 `data` 的结构 |
| `sender` | 主程序发送时填 `"nashell"`，插件发送时填插件名 |
| `data` | 消息载荷，随 `type` 不同而变化 |

---

## 四、消息类型详述

### 4.1 Call — 主程序调用插件命令

当用户在 REPL 中输入匹配插件注册的命令时，主程序向插件发送 call 消息。

**方向**：NaShell → 插件

```json
{
    "type": "call",
    "sender": "nashell",
    "data": {
        "command": "demo",
        "mode": "normal",
        "level": "normal",
        "params": ["--name", "World"],
        "long_argument": "多行文本内容..."
    }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `command` | string | 命令名，**已小写化** |
| `mode` | string | 子命令/模式。若用户未指定则为 `"normal"`。**注意**：NaShell 不为插件命令做 mode 提取（`known_modes` 为空），插件的 mode 信息通常保留在 `params[0]` 中，需由插件自行解析 |
| `level` | string | 命令级别：`"normal"` 或 `"system"` |
| `params` | string[] | 命令行参数列表。**包含可能的 mode 名称**（如 `["stream", "-c", "5"]`） |
| `long_argument` | string \| null | `@/` 或空行后的多行长参数，无则为 `null` |

#### Mode 解析建议

由于 NaShell 不为插件命令做 mode 提取，`params[0]` 可能包含实际的 mode 名称。
推荐在插件中按以下规则解析：

```python
mode = data.get("mode", "normal")
params = data.get("params", [])

# 若 mode 为默认 "normal" 且 params[0] 不是 option 标志，
# 则视作实际 mode 并从 params 中移除
if mode == "normal" and params and not params[0].startswith("-"):
    mode = params[0].lower()
    params = params[1:]
```

### 4.2 Response — 插件返回执行结果

插件在收到 call 后可以发送零或多条 response 消息，用于流式输出或请求 toExec。

**方向**：插件 → NaShell

```json
{
    "type": "response",
    "sender": "my_plugin",
    "data": {
        "streaming": true,
        "out_content": "正在处理第 3/10 项...",
        "out_prompt": "@my_plugin #>>",
        "is_print": true,
        "to_exec": [],
        "exec_result": null
    }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `streaming` | bool | 为 `true` 表示后续还有 response 消息（尚未结束） |
| `out_content` | string | 输出内容。**可包含 ANSI 彩色码**（如 `\x1b[32m绿色文本\x1b[0m`）。NaShell 原样保留不转义 |
| `out_prompt` | string \| null | 输出提示符。若 `is_print` 为 true 且此字段有值，则在 `out_content` 前打印提示符（如 `@my_plugin #>>`） |
| `is_print` | bool | 是否**实时打印**到终端。为 `true` 时，`out_content` 立即写入 stdout（不等待全部完成）。这是实现流式输出的关键字段 |
| `to_exec` | string[] | 要求主程序代为执行的命令列表。每条命令走与用户输入完全相同的解析+执行流程。非空时，主程序执行后会将结果填回 `exec_result` 发回 |
| `exec_result` | string[] \| null | `to_exec` 的执行结果数组（主程序填充后发回）。数组顺序与 `to_exec` 一一对应。仅出现在主程序发回给插件的 response 中 |

#### 流式输出示例

```
插件                         NaShell
  │                             │
  │── response #1 ──────────→  │   streaming=true, is_print=true
  │   "正在分析..."              │   → 立即打印到终端
  │                             │
  │── response #2 ──────────→  │   streaming=true, is_print=true
  │   "完成 50%"                │   → 立即打印到终端
  │                             │
  │── response #3 ──────────→  │   streaming=false, is_print=true
  │   "全部完成"                │   → 立即打印到终端
  │                             │
  │── off ──────────────────→  │   结束本次调用
```

### 4.3 Off — 结束通知

插件处理完毕后发送，标志一次 call 会话结束。主程序收到 off 后不会继续等待该插件的输出。

**方向**：插件 → NaShell

```json
{
    "type": "off",
    "sender": "my_plugin",
    "data": {
        "to_exec": [],
        "out_content": "操作完成",
        "out_prompt": "@my_plugin #>>",
        "is_print": true
    }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `to_exec` | string[] | 结束前要求执行的命令列表。与 response 中的 toExec 不同，**off 中的 toExec 结果不回传给插件**（因为连接即将关闭） |
| `out_content` | string | 结束前最后一段输出 |
| `out_prompt` | string \| null | 输出提示符 |
| `is_print` | bool | 是否打印 |

**注意**：off 消息发送后，本次 call 会话结束。插件进程继续存活，等待下一个 call。

#### toExec 在 response 和 off 中的区别

| | response 中的 to_exec | off 中的 to_exec |
|---|---|---|
| 结果是否回传插件 | **是**。主程序执行后发回含 `exec_result` 的 response | **否**。结果直接输出到终端 |
| 适用场景 | 插件需要了解执行结果并继续处理 | 清理收尾操作，插件不需要结果 |
| 插件后续 | 继续等待 exec_result response | call 会话结束 |

### 4.4 Broadcast — 广播事件

主程序向所有注册了 `is_broadcast: true` 的插件发送事件通知。

**方向**：NaShell → 插件

```json
{
    "type": "broadcast",
    "sender": "nashell",
    "data": {
        "event": "shell_state_changed",
        "payload": {
            "name": "main",
            "path": "/home/user/new_dir"
        }
    }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `event` | string | 事件名称。当前支持：`shell_state_changed`、`cwd_changed` |
| `payload` | object | 事件载荷，结构依事件类型而定 |

**注意**：广播消息无回复机制。插件收到后自主处理，不向主程序回复。

---

## 五、插件生命周期

### 5.1 启动

1. NaShell 启动时扫描 `~/.config/nashell/plugins/` 下各子目录的 `manifest.json`
2. 解析成功的插件被注册到命令表
3. `is_broadcast: true` 的插件**立即启动并保活**
4. 非 broadcast 插件在首次被调用时启动并保活

### 5.2 一次 Call 会话

```
┌─ 主程序发送 call 消息到插件 stdin
│
├─ 插件处理（可发送零或多条 response）
│   ├─ response（streaming=true, is_print=true）→ 主程序实时打印
│   ├─ response（含 to_exec）→ 主程序执行命令 → 发回 exec_result
│   └─ ...（插件可继续发送 response）
│
└─ 插件发送 off → 本次 call 结束
   └─ 插件进程继续存活，等待下一个 call
```

**关键规则**：
- 插件在收到 call 后**必须**最终发送 off（即使内容为空）
- 插件在发送 off 后继续等待下一个 call（不要退出）
- 插件进程应是一个持久循环：`while True: read call → process → send off`

### 5.3 关闭

- 主程序退出时向所有插件发送 SIGTERM
- 超时（30 秒）未退出则强制 kill
- 插件检测到 stdin EOF 时应自行退出

### 5.4 超时机制

| 场景 | 超时 | 行为 |
|------|------|------|
| 插件 30 秒无响应 | 30 秒 | 强制关闭插件进程，报错 |
| Shell 命令执行超时 | `shell.timeout_secs`（默认 120 秒） | 终止命令执行 |

---

## 六、toExec 机制详解

toExec 是插件系统的核心特性之一，允许插件**请求主程序代为执行任意命令**（shell 命令或 NaCommand），并获取执行结果。

### 执行流程

```
1. 插件发送 response（含 to_exec=["cmd1", "cmd2"]，streaming=true）
      ↓
2. 主程序按顺序执行每条命令（与用户在 REPL 中输入的效果完全一致）
      ↓
3. 主程序收集所有命令的输出，填入 exec_result 数组
      ↓
4. 主程序发送 response（含 exec_result）给插件
      ↓
5. 插件接收 exec_result，继续处理或发送 off
```

### 重要细节

- **执行流程与用户输入一致**：toExec 中的命令经过完整的解析→分派→执行流程，支持 `!!@Bash:`、`!@Write:` 等所有 NaCommand
- **单条命令失败不影响其他**：若某条命令执行失败，错误信息会作为字符串放入对应位置的 exec_result，不会中断整个批次
- **安全拦截仍然生效**：匹配 `deny_patterns` 的命令会被拒绝

### 递归深度限制

为防止无限递归，toExec 设置了最大递归深度 **3**：

| 深度 | 说明 |
|------|------|
| 0 | 用户直接输入触发的命令 |
| 1 | 插件 toExec 触发的命令 |
| 2 | 上述命令触发的插件 toExec |
| 3 | 最后一层允许的 toExec |
| > 3 | NaCommand 被拒绝（返回错误），仅允许纯 shell 命令 |

### 使用示例

```python
# 请求主程序执行一条 shell 命令并获取结果
send_response(
    streaming=True,
    content="正在执行 ls...",
    prompt=None,
    is_print=True,
    to_exec=["ls -la"]
)
# 主程序执行后发回含 exec_result 的 response
# 插件在 main loop 中接收并处理 exec_result
```

---

## 七、完整开发示例

以下是一个最简 Python 插件实现，支持一个命令、两种模式。

### manifest.json

```json
{
    "name": "hello_plugin",
    "exec": "python3 /path/to/hello_plugin/plugin.py",
    "nacommands": {
        "hello": {
            "level": "normal",
            "long_argument": true
        }
    },
    "is_broadcast": false
}
```

### plugin.py

```python
#!/usr/bin/env python3
"""最小 NaShell 插件示例."""
import json
import sys

# ── ANSI 快捷方式 ──
GREEN = "\x1b[32m"
CYAN = "\x1b[36m"
RESET = "\x1b[0m"


def send_response(streaming, content, prompt=None, is_print=True,
                  to_exec=None):
    """发送一条 response."""
    msg = {
        "type": "response",
        "sender": "hello_plugin",
        "data": {
            "streaming": streaming,
            "out_content": content,
            "out_prompt": prompt,
            "is_print": is_print,
            "to_exec": to_exec or [],
            "exec_result": None
        }
    }
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def send_off(content="", prompt=None, is_print=True, to_exec=None):
    """发送 off 并结束本次 call."""
    msg = {
        "type": "off",
        "sender": "hello_plugin",
        "data": {
            "to_exec": to_exec or [],
            "out_content": content,
            "out_prompt": prompt,
            "is_print": is_print
        }
    }
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main():
    while True:
        # 读取一条有效消息（跳过空行和非 call/response 消息）
        line = ""
        for raw in sys.stdin:
            raw = raw.strip()
            if not raw:
                continue
            try:
                msg = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if msg.get("type") in ("call", "response"):
                line = raw
                break
        if not line:
            sys.exit(0)  # stdin 关闭

        msg = json.loads(line)
        msg_type = msg["type"]
        data = msg.get("data", {})

        # ── 处理 response（含 exec_result）──
        if msg_type == "response":
            exec_result = data.get("exec_result")
            if exec_result:
                # 这是之前 toExec 请求的返回结果
                lines = [f"{GREEN}收到 {len(exec_result)} 条结果:{RESET}"]
                for i, r in enumerate(exec_result):
                    lines.append(f"  [{i + 1}] {r}")
                send_off(content="\n".join(lines), prompt="@hello #>>")
            continue

        # ── 处理 call ──
        if msg_type != "call":
            continue

        command = data["command"]
        mode = data.get("mode", "normal")
        params = data.get("params", [])
        long_arg = data.get("long_argument")

        # Mode 解析（从 params[0] 提取实际 mode）
        if mode == "normal" and params and not params[0].startswith("-"):
            mode = params[0].lower()
            params = params[1:]

        # ── hello 命令 ──
        if command == "hello":
            if mode == "help":
                send_off(
                    content=f"{CYAN}Hello 命令{RESET}\n问候世界。",
                    prompt="@hello #>>"
                )
            elif mode == "exec_ls":
                # 请求主程序执行 ls
                send_response(
                    streaming=True,
                    content=f"{CYAN}请求执行 ls...{RESET}",
                    is_print=True,
                    to_exec=["ls"]
                )
                # 主程序执行后发回 exec_result，在 response 分支处理
            else:
                # 默认模式：简单问候
                name = params[0] if params else "World"
                greeting = f"{GREEN}Hello, {name}!{RESET}"
                if long_arg:
                    greeting += f"\n收到长参数: {long_arg}"
                send_off(content=greeting, prompt="@hello #>>")


if __name__ == "__main__":
    main()
```

### 测试命令

```
!@Hello:                      # 默认问候
!@Hello:Alice                 # 带名字
!@Hello:Exec_ls               # 请求执行 ls
!@Hello:Help                  # 帮助
```

---

## 八、最佳实践

### 输出规范

- **使用 ANSI 彩色码**增强可读性，NaShell 原样保留不转义
- **输出提示符**（`out_prompt`）建议格式：`@plugin_name #>>`
- **流式输出**：设置 `streaming=true` 让主程序知道后续还有消息
- **实时打印**：设置 `is_print=true` 让内容立即显示

### 错误处理

- 处理所有可能的 JSON 解析异常
- 遇到无法处理的 call 时发送 off 说明错误
- 不要在插件中 `panic` 或未捕获异常退出
- 使用 `sys.stderr` 输出调试日志（不要写入 stdout）

### 性能

- 避免在插件中执行耗时操作阻塞主程序
- 复杂任务使用 toExec 委托给主程序执行
- 插件进程保活，避免每次 call 重新启动进程

### 调试

- 设置环境变量 `RUST_LOG=debug` 查看主程序的插件通信日志
- 插件可将调试信息写入文件或 stderr
- 测试时可直接向插件 stdin 写入 NDJSON 进行独立测试：
  ```bash
  echo '{"type":"call","sender":"nashell","data":{"command":"hello","mode":"normal","level":"normal","params":["World"],"long_argument":null}}' | python3 plugin.py
  ```

---

## 九、常量与限制

| 常量 | 值 | 说明 |
|------|------|------|
| `TOEXEC_MAX_DEPTH` | 3 | toExec 最大递归深度 |
| `PLUGIN_TIMEOUT_SECS` | 30 | 插件无响应超时 |
| `DEFAULT_SHELL_TIMEOUT_SECS` | 120 | Shell 命令默认超时 |

---

## 十、参考实现

完整的测试插件位于项目 `test_plugins/demo_plugin/`，演示了以下功能：

| 功能 | 命令/模式 | 说明 |
|------|-----------|------|
| 简单输出 | `Demo:Echo` | 彩色问候 + long_argument |
| 流式输出 | `Demo:Stream` | 多段实时彩色进度条 |
| 单命令委托 | `Demo:Exec` | 发送 toExec 并接收 exec_result 反馈 |
| 多命令委托 | `Demo:MultiExec` | 批量 toExec + 格式化输出结果 |
| 帮助 | `Demo:Help` / `SysInfo:Help` | ANSI 彩色帮助文本 |
| 系统信息 | `SysInfo:` | 默认 / 详细 / 帮助三种模式 |
| 双命令级别 | Demo(Normal) + SysInfo(System) | 不同级别的命令示例 |
