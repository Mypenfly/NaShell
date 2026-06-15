#!/usr/bin/env python3
"""NaShell 演示插件 —— 展示插件系统的核心功能。

支持命令：
  !@Demo:Echo <name>             简单输出反馈
  !@Demo:Stream [-c <n>]         流式进度条输出
  !@Demo:Exec <cmd>              toExec 单命令 + 结果回传
  !@Demo:MultiExec <cmd...>      toExec 多命令 + 格式化结果
  !@Demo:Silent <cmd>            toExec is_print=false 演示
  !@Demo:Confirm                 交互式输入（get_input）
  !@Demo:Help                    帮助信息
"""

import json
import sys
import time

# ── ANSI 快捷方式 ──
R = "\x1b[0m"
B = "\x1b[1m"
C = "\x1b[36m"
G = "\x1b[32m"
Y = "\x1b[33m"
M = "\x1b[35m"
BC = "\x1b[96m"
BG = "\x1b[92m"
BY = "\x1b[93m"

SENDER = "demo_plugin"


def _send(msg: dict):
    sys.stdout.write(json.dumps(msg, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def send_response(streaming: bool, content: str = "", prompt: str | None = None,
                  prompt_fg: str = "gray", is_print: bool = True,
                  to_exec: dict | None = None, get_input: dict | None = None):
    """发送 response 消息。
    
    to_exec 格式: {"execs": [...], "is_print": bool, "timeout": int}
    """
    _send({
        "type": "response",
        "sender": SENDER,
        "data": {
            "streaming": streaming,
            "out_content": content,
            "out_prompt": prompt,
            "prompt_fg": prompt_fg,
            "is_print": is_print,
            "to_exec": to_exec,
            "exec_result": None,
            "get_input": get_input,
            "user_input": None,
        }
    })


def send_off(content: str = "", prompt: str | None = None,
             prompt_fg: str = "gray", is_print: bool = True,
             to_exec: dict | None = None):
    """发送 off 消息结束本次 call。
    
    to_exec 格式: {"execs": [...], "is_print": bool, "timeout": int}
    """
    _send({
        "type": "off",
        "sender": SENDER,
        "data": {
            "to_exec": to_exec,
            "out_content": content,
            "out_prompt": prompt,
            "prompt_fg": prompt_fg,
            "is_print": is_print,
        }
    })


def recv_message() -> dict | None:
    """从 stdin 读取一条有效的 call 或 response 消息。"""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("type") in ("call", "response"):
            return msg
    return None


# ── 命令处理 ──

def handle_demo(mode: str, params: list, long_arg: str | None):
    if mode == "help":
        send_off(content=HELP, prompt="@demo #>", prompt_fg="gray")
        return

    if mode in ("echo", "normal"):
        name = params[0] if params else "NaShell"
        text = f"{BG}Hello, {name}!{R}"
        if long_arg:
            text += f"\n{BC}long_argument{R}: {long_arg[:200]}"
        send_off(content=text, prompt="@demo #>", prompt_fg="gray")
        return

    if mode == "stream":
        count = 5
        for i, p in enumerate(params):
            if p in ("-c", "--count") and i + 1 < len(params):
                try:
                    count = int(params[i + 1])
                except ValueError:
                    pass

        send_response(True, f"{BC}── 流式输出 ({count} 条) ──{R}\n",
                      prompt="@demo #>", prompt_fg="gray")
        for i in range(count):
            bar = "█" * (i + 1) + "░" * (count - i - 1)
            color = [BC, BG, BY, M, C][i % 5]
            send_response(i < count - 1,
                          f"  [{i + 1}/{count}] {color}{bar}{R}",
                          prompt=None, is_print=True)
            time.sleep(0.15)
        send_off(f"{BG}── 流式完成 ──{R}", prompt="@demo #>", prompt_fg="gray")
        return

    if mode == "exec":
        # toExec 单命令：is_print=True → 结果实时显示在终端
        cmd = params[0] if params else "echo 'no command'"
        send_response(True, f"{BC}→ toExec: {cmd}{R}", is_print=True,
                      to_exec={"execs": [cmd], "is_print": True, "timeout": 90})
        return  # 结果在 response 分支处理

    if mode == "multiexec":
        # toExec 多命令：is_print=True → 每条结果实时显示
        cmds = params if params else ["echo a", "echo b", "echo c"]
        send_response(True,
                      f"{BC}→ 批量 toExec ({len(cmds)} 条){R}",
                      is_print=True,
                      to_exec={"execs": cmds, "is_print": True, "timeout": 90})
        return

    if mode == "silent":
        # toExec is_print=False → 结果不回显，只捕获回传
        cmd = params[0] if params else "echo 'secret result'"
        send_response(True,
                      f"{BC}→ 静默 toExec (is_print=false): {cmd}{R}",
                      is_print=True,
                      to_exec={"execs": [cmd], "is_print": False, "timeout": 30})
        return  # 静默执行的结果在 response 分支接收

    if mode == "confirm":
        send_response(True, "", is_print=False,
                      get_input={
                          "pre_content": "这是一个危险操作",
                          "pre_fg": "gray",
                          "input_prompt": "确认继续? (y/n) > ",
                          "input_fg": "bright_yellow",
                      })
        return  # 用户输入在 response 分支处理

    # 默认
    send_off(f"{BG}Demo 默认{R}\n  参数: {params}", prompt="@demo #>", prompt_fg="gray")


HELP = f"""
{B}{BC}Demo{R} — 演示插件 (Normal 级)

{B}模式:{R}
  {G}echo{R}        简单问候（args[0] 为名字）
  {G}stream{R}      流式进度条 (-c <n>)
  {G}exec{R}        toExec 单命令执行（is_print=true，结果实时显示）
  {G}multiexec{R}   toExec 多命令执行（is_print=true）
  {G}silent{R}      toExec 静默执行（is_print=false，结果回传后由插件格式化输出）
  {G}confirm{R}     交互式输入示例（get_input）
  {G}help{R}        本帮助

{B}示例:{R}
  !@Demo:Echo Alice
  !@Demo:Stream -c 10
  !@Demo:Exec ls -la
  !@Demo:MultiExec echo A echo B
  !@Demo:Silent date
  !@Demo:Confirm
"""


# ── 主循环 ──

def main():
    while True:
        msg = recv_message()
        if msg is None:
            sys.exit(0)

        msg_type = msg.get("type", "")
        data = msg.get("data", {})

        # ── 处理 response（含 exec_result 或 user_input）──
        if msg_type == "response":
            exec_result = data.get("exec_result")
            user_input = data.get("user_input")

            if exec_result:
                # 收集到 toExec 的执行结果，格式化后输出
                lines = [f"{BG}── toExec 结果 ({len(exec_result)} 条) ──{R}"]
                for i, r in enumerate(exec_result):
                    if r.strip():
                        lines.append(f"  {BC}[{i + 1}]{R} {r.strip()}")
                send_off("\n".join(lines), prompt="@demo #>", prompt_fg="gray")
                continue

            if user_input is not None:
                lines = [
                    f"{BG}── 收到用户输入 ──{R}",
                    f"  {BC}{user_input}{R}",
                ]
                send_off("\n".join(lines), prompt="@demo #>", prompt_fg="gray")
                continue

            continue

        # ── 处理 call ──
        if msg_type != "call":
            continue

        command = data.get("command", "")
        mode = data.get("mode", "normal")
        params = data.get("params", [])
        long_arg = data.get("long_argument")

        # 从 params[0] 提取实际 mode（插件命令 known_modes 为空，mode 在 params[0]）
        if mode == "normal" and params and not params[0].startswith("-"):
            mode = params[0].lower()
            params = params[1:]

        if command == "demo":
            handle_demo(mode, params, long_arg)
        else:
            send_off(f"{R}未知命令: {command}", prompt="@demo #>", prompt_fg="gray")


if __name__ == "__main__":
    main()
