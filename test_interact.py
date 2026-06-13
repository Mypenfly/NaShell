#!/usr/bin/env python3
"""
NaShell 交互 / 流式输出测试脚本

测试场景：
  1. 普通输出
  2. 流式进度输出 (实时 flush)
  3. 交互式输入 (input)
  4. 混合场景

用法：
  python3 test_interact.py
  python3 -c "..."  (在 nashell 中直接输命令也行)
"""

import sys
import time


def test_simple():
    """1. 最简单输出 — 验证基础管道"""
    print("Hello from Python test script!")


def test_streaming():
    """2. 流式输出 — 验证逐 flush 的输出能实时看到（不等到最后才一次吐出）"""
    print("Streaming test: ", end="", flush=True)
    for i in range(8):
        time.sleep(0.25)
        print(".", end="", flush=True)
    print(" done!")
    print("If you saw dots appearing one by one, streaming works.")


def test_countdown():
    """3. 倒计时 — 更明显的流式效果"""
    print("Countdown:", flush=True)
    for i in range(5, 0, -1):
        print(f"  {i}...", flush=True)
        time.sleep(0.3)
    print("  Liftoff!")


def test_interactive():
    """4. 交互输入 — 验证 stdin 能正常传给子进程"""
    print()
    name = input("Enter your name: ")
    print(f"Hello, {name}!")

    answer = input("Continue? [y/n]: ")
    if answer.lower() != 'y':
        print("Aborted by user.")
        return
    print("Continuing...")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--stream-only":
        test_countdown()
        return

    if len(sys.argv) > 1 and sys.argv[1] == "--interact-only":
        test_interactive()
        return

    test_simple()
    print()
    test_streaming()
    print()
    test_countdown()
    test_interactive()
    print()
    print("All tests completed successfully!")


if __name__ == "__main__":
    main()
