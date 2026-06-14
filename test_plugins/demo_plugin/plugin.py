#!/usr/bin/env python3
"""NaShell 测试插件 —— 演示插件系统的全部功能。

支持两个命令：
  - demo (Normal 级)   — 演示 echo / stream / exec / multiexec / help
  - sysinfo (System 级) — 系统信息查询 / help

通信协议：
  - stdin 读取 NDJSON 消息（每行一条完整 JSON）
  - stdout 写入 NDJSON 消息
  - type="call"  → 处理
  - type="response" → 输出 (streaming 控制是否还有后续)
  - type="off"     → 结束通知
"""

import json
import sys
import os
import time
import platform

# ── ANSI 颜色工具 ──────────────────────────────────────────────

def ansi(code: int, text: str) -> str:
    """包裹 ANSI 转义序列."""
    return f"\x1b[{code}m{text}\x1b[0m"

BRIGHT = "\x1b[1m"
RESET = "\x1b[0m"
RED = "\x1b[31m"
GREEN = "\x1b[32m"
YELLOW = "\x1b[33m"
BLUE = "\x1b[34m"
MAGENTA = "\x1b[35m"
CYAN = "\x1b[36m"
WHITE = "\x1b[37m"
BRIGHT_GREEN = "\x1b[92m"
BRIGHT_YELLOW = "\x1b[93m"
BRIGHT_CYAN = "\x1b[96m"


def send_response(streaming: bool, content: str, prompt: str | None = None,
                  is_print: bool = True, to_exec: list | None = None,
                  exec_result: list | None = None):
    """发送一条 response 消息到 stdout."""
    msg = {
        "type": "response",
        "sender": "demo_plugin",
        "data": {
            "streaming": streaming,
            "out_content": content,
            "out_prompt": prompt,
            "is_print": is_print,
            "to_exec": to_exec or [],
            "exec_result": exec_result
        }
    }
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def send_off(content: str = "", prompt: str | None = None,
             is_print: bool = True, to_exec: list | None = None):
    """发送一条 off 消息并退出."""
    msg = {
        "type": "off",
        "sender": "demo_plugin",
        "data": {
            "to_exec": to_exec or [],
            "out_content": content,
            "out_prompt": prompt,
            "is_print": is_print
        }
    }
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def fail(reason: str):
    """发送错误 off 消息."""
    send_off(
        content=f"{RED}✗ 错误: {reason}{RESET}",
        prompt="@demo_plugin #>>",
        is_print=True
    )
    sys.exit(1)


# ── Demo 命令处理 ───────────────────────────────────────────────

def handle_demo(mode: str, params: list, long_argument: str | None):
    """处理 demo 命令的各种模式."""

    if mode == "help":
        send_off(
            content=demo_help(),
            prompt="@demo_plugin #>>",
            is_print=True
        )
        return

    if mode in ("echo", "normal"):
        # 简单输出反馈
        name = ""
        for i, p in enumerate(params):
            if p in ("-n", "--name") and i + 1 < len(params):
                name = params[i + 1]
                break
        greeting = f"Hello, {name}!" if name else "Hello, NaShell!"
        if long_argument:
            greeting += f"\n收到的长参数: {long_argument[:200]}"

        send_off(
            content=f"{BRIGHT_GREEN}{greeting}{RESET}",
            prompt="@demo_plugin #>>",
            is_print=True
        )
        return

    if mode == "stream":
        # 流式输出
        count = 5
        interval = 0.2
        for i, p in enumerate(params):
            if p in ("-c", "--count") and i + 1 < len(params):
                try:
                    count = int(params[i + 1])
                except ValueError:
                    pass

        send_response(
            streaming=True,
            content=f"{BRIGHT_CYAN}┌─ 流式输出开始 ──────────────────────{RESET}\n",
            prompt=None,
            is_print=True
        )

        for i in range(count):
            bar = "█" * (i + 1) + "░" * (count - i - 1)
            color = [BRIGHT_CYAN, BRIGHT_GREEN, BRIGHT_YELLOW, MAGENTA, BLUE][i % 5]
            send_response(
                streaming=(i < count - 1),
                content=f"  [{i + 1}/{count}] {color}{bar}{RESET} 数据处理中...",
                prompt=None,
                is_print=True
            )
            time.sleep(interval)

        send_off(
            content=f"{BRIGHT_GREEN}└─ 流式输出完成 ({count} 条记录){RESET}",
            prompt=None,
            is_print=True
        )
        return

    if mode == "exec":
        # 单个 toExec 命令 — 通过 response+to_exec 让插件收到执行结果反馈
        cmd = params[0] if params else "echo 'no command provided'"
        send_response(
            streaming=True,
            content=f"{BRIGHT_CYAN}→ 请求执行命令: {cmd}{RESET}",
            prompt=None,
            is_print=True,
            to_exec=[cmd]
        )
        # 主程序执行 to_exec 后发回 exec_result，插件 main loop 接收并格式化输出
        return

    if mode == "multiexec":
        # 多个 toExec 命令 + 交互式 toExec（先请求再送回结果）
        commands = params if params else ["echo step1", "echo step2", "echo step3"]

        send_response(
            streaming=True,
            content=f"{BRIGHT_CYAN}┌─ 开始批量执行 ({len(commands)} 条命令){RESET}",
            prompt=None,
            is_print=True,
            to_exec=commands
        )
        # 注意：主程序会执行 to_exec，然后发回包含 exec_result 的 response
        # 插件收到后继续处理
        return

    # 默认: echo 模式
    send_off(
        content=f"{BRIGHT_GREEN}Demo 默认响应{WHITE}\n参数: {params}{RESET}",
        prompt="@demo_plugin #>>",
        is_print=True
    )


def demo_help() -> str:
    """Demo 命令帮助文本."""
    return f"""
{BRIGHT}{BRIGHT_CYAN}Demo{WHITE} — 演示命令 (Normal 级)

{BRIGHT}模式:{WHITE}
  {GREEN}echo / normal{RESET}       简单输出反馈
    -n, --name <name>   指定问候名称
    long_argument        可附加长文本
    {BRIGHT_YELLOW}示例:{RESET} !@Demo:Echo -n World @/
                      >> 这是长参数

  {GREEN}stream{RESET}              流式输出示例
    -c, --count <n>     输出条数 (默认 5)
    {BRIGHT_YELLOW}示例:{RESET} !@Demo:Stream -c 10

  {GREEN}exec{RESET}                请求执行单个 shell 命令
    <cmd>                要执行的命令
    {BRIGHT_YELLOW}示例:{RESET} !@Demo:Exec ls -la

  {GREEN}multiexec{RESET}           请求执行多个 shell 命令
    <cmd1> <cmd2> ...    命令列表
    {BRIGHT_YELLOW}示例:{RESET} !@Demo:MultiExec echo A echo B echo C

  {GREEN}help{RESET}                显示本帮助
{BRIGHT_YELLOW}注意:{RESET} exec/multiexec 中的 to_exec 命令由主程序代为执行，结果回传。
"""


# ── SysInfo 命令处理 ────────────────────────────────────────────

def handle_sysinfo(mode: str, params: list, long_argument: str | None):
    """处理 sysinfo 命令的各种模式."""

    if mode == "help":
        send_off(
            content=sysinfo_help(),
            prompt="@demo_plugin #>>",
            is_print=True
        )
        return

    # 默认: 显示系统信息
    info_lines = [
        f"{BRIGHT}{BRIGHT_CYAN}╔══════════════════════════════════════╗",
        f"║        System Information          ║",
        f"╚══════════════════════════════════════╝{RESET}",
        "",
        f"  {BRIGHT_GREEN}系统:{WHITE}     {platform.system()} {platform.release()}",
        f"  {BRIGHT_GREEN}架构:{WHITE}     {platform.machine()}",
        f"  {BRIGHT_GREEN}主机名:{WHITE}   {platform.node()}",
        f"  {BRIGHT_GREEN}Python:{WHITE}   {platform.python_version()}",
        f"  {BRIGHT_GREEN}CWD:{WHITE}      {os.getcwd()}",
        f"  {BRIGHT_GREEN}PID:{WHITE}      {os.getpid()}",
        f"  {BRIGHT_GREEN}用户:{WHITE}     {os.environ.get('USER', 'unknown')}",
    ]

    # 如果指定了 --detail，显示更多信息
    show_detail = any(p in ("-d", "--detail") for p in params)
    if show_detail:
        info_lines.append("")
        info_lines.append(f"  {BRIGHT_YELLOW}── 环境变量 (筛选) ──{RESET}")
        for key in sorted(os.environ):
            if key.startswith(("HOME", "PATH", "SHELL", "USER", "LANG", "TERM")):
                val = os.environ[key]
                if len(val) > 80:
                    val = val[:77] + "..."
                info_lines.append(f"    {GREEN}{key}{WHITE}={val}{RESET}")

    send_off(
        content="\n".join(info_lines),
        prompt="@sysinfo #>>",
        is_print=True
    )


def sysinfo_help() -> str:
    """SysInfo 命令帮助文本."""
    return f"""
{BRIGHT}{BRIGHT_CYAN}SysInfo{WHITE} — 系统信息查询 (System 级)

{BRIGHT}参数:{WHITE}
  -d, --detail   显示详细环境变量信息
  help           显示本帮助

{BRIGHT}模式:{WHITE}
  {GREEN}(默认){RESET}       显示基本系统信息
  {GREEN}help{RESET}         显示本帮助

{BRIGHT_YELLOW}示例:{RESET}
  !!@SysInfo:
  !!@SysInfo: -d
  !!@SysInfo:Help
"""


# ── 主循环 ──────────────────────────────────────────────────────

def recv_message() -> dict | None:
    """读取一条有效的 NDJSON 消息。跳过非 call/response 类型和空行。"""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        msg_type = msg.get("type", "")
        if msg_type in ("call", "response"):
            return msg
        continue
    return None


def main():
    while True:
        msg = recv_message()
        if msg is None:
            sys.exit(0)

        msg_type = msg.get("type", "")
        data = msg.get("data", {})

        if msg_type == "response":
            exec_result = data.get("exec_result")
            if exec_result:
                # This is the result of a previous toExec request.
                # Build a clear formatted output with each result in its own block.
                lines = []
                lines.append(f"{BRIGHT_GREEN}└─ 批量执行完成: 收到 {len(exec_result)} 条结果{RESET}")
                for i, r in enumerate(exec_result):
                    lines.append(f"")
                    lines.append(f"  {BRIGHT_CYAN}── 命令 [{i + 1}/{len(exec_result)}] 执行结果 ──{RESET}")
                    # Don't truncate — show full output with proper indentation
                    for line in r.split('\n'):
                        # Strip trailing whitespace but preserve formatting
                        lines.append(f"  {line.rstrip()}")
                send_off(
                    content="\n".join(lines),
                    prompt="@demo_plugin #>>",
                    is_print=True
                )
            else:
                # Other response: ignored (plugin doesn't handle streaming exec_result followups)
                pass
            continue

        if msg_type != "call":
            continue

        command = data.get("command", "")
        mode = data.get("mode", "normal")
        params = data.get("params", [])
        long_argument = data.get("long_argument")

        # If mode is "normal" and params[0] looks like a mode name (not a flag),
        # extract it as the actual mode
        if mode == "normal" and params and not params[0].startswith("-"):
            mode = params[0].lower()
            params = params[1:]

        if command == "demo":
            handle_demo(mode, params, long_argument)
            continue

        elif command == "sysinfo":
            handle_sysinfo(mode, params, long_argument)
            continue

        else:
            fail(f"未知命令: {command}")

    sys.exit(0)


if __name__ == "__main__":
    main()
