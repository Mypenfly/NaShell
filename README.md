# NaShell

A "pseudo-shell" built atop existing shells (nushell/bash), designed for both humans and LLM agents. It introduces semantic-level special commands (`NaCommand`) that seamlessly interoperate with conventional shell commands.

## Core Design

- **Semantic NaCommands** — structured commands with typed parameters, long-argument support, and mode dispatch. Escape-free multi-line input via `@/` delimiter.
- **Shell-compatible** — all standard shell commands (`ls`, `git`, `curl`, `vim` etc.) work natively through the underlying shell.
- **Dual-mode execution** — direct terminal mode for interactive TUI programs, captured PTY mode for pipelines and formatted output.
- **Plugin system** — NDJSON-based communication protocol, streaming output, toExec delegation, broadcast events.
- **Async shells** — background execution via `@/Async(name)`, with independent shell environments and output pools.
- **Rich error handling** — NaCommand-level format validation, level checking, fuzzy command suggestions with green ANSI hints.
- **Signal handling** — SIGINT (Ctrl+C) for command interruption, double Ctrl+C for force quit, graceful shutdown on SIGTERM/SIGHUP.

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/nashell
```

```text
~/projects/nashell |> ls -la         # Shell command — works natively
~/projects/nashell |> ls | grep src  # Pipes work too
~/projects/nashell |> !@Write:./hello.py @/   # NaCommand: write a file
                     >> print("Hello, NaShell!")
                     >>
~/projects/nashell |> !!@Bash: ls -la         # Bash shortcut
~/projects/nashell |> echo done @/Async(back)  # Async execution
~/projects/nashell |> !!@Shell:               # Shell management
```

## Architecture

```
NaShell Process
├─ REPL Frontend (rustyline)
│   ├─ Multi-line input with @/ delimiter
│   ├─ ANSI-colored prompt rendering
│   └─ History management
├─ Command Parser
│   ├─ Lexer: tokenize !@/!!@/@//pipes/quoted strings
│   ├─ Async marker detection (@/Async(name))
│   ├─ Long-argument extraction (@/ or blank-line)
│   └─ Pipeline splitting (|)
├─ Execution Engine
│   ├─ Direct mode: stdin/stdout/stderr inherit → interactive programs
│   ├─ Captured mode: script -e -q -c → PTY-aware output capture
│   ├─ NaCommand dispatch: builtin → config → plugin lookup
│   └─ Safety check: deny_patterns matching
├─ Shell Manager
│   ├─ Main shell (cwd-synced with Rust process)
│   ├─ Async shells (background threads, independent cwd)
│   └─ Shell pools (accumulated output for async exec)
├─ Plugin Manager
│   ├─ Child process lifecycle (start/send/recv/stop)
│   ├─ NDJSON frame protocol over stdin/stdout
│   ├─ toExec recursion engine (depth-limited)
│   └─ Broadcast event channel
└─ Configuration Loader
    ├─ ~/.config/nashell/config.kdl (KDL format)
    ├─ ~/.config/nashell/plugins/ (manifest.json scanning)
    └─ Alias expansion
```

## Builtin NaCommands

| Command | Level | Description |
|---------|-------|-------------|
| `!@Write:` | Normal | Write file content. `path` from args, content from `@/` long-argument. |
| `!@Read:` | Normal | Read file or directory. Syntax highlighting for files, tree view for dirs. |
| `!!@Bash:` | System | Execute via `bash -c`. Highest parse priority, bypasses all other rules. |
| `!!@Shell:` | System | Manage shell threads. Modes: (default), Watch, Destroy, Switch. |
| `!@NaCmds:` | System | List all registered commands. Modes: (default table), Detail (with help), `-j/--json`. |

## Plugin System

Plugins are standalone executables communicating via NDJSON over stdin/stdout.

See **[Plugin Development Guide](docs/plugin_dev.md)** for protocol details, lifecycle, and examples.

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

Key features:
- **Language-agnostic** — any language with JSON + stdin/stdout
- **Streaming output** — multi-segment real-time responses
- **toExec delegation** — plugins request the host to execute commands
- **Broadcast events** — subscribe to `cwd_changed` / `shell_state_changed`

## Configuration

Configuration file: `~/.config/nashell/config.kdl` (KDL format). Falls back to built-in defaults if missing.

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

## Input Syntax

```
// Shell command — passed to nushell/bash
ls -la

// NaCommand (Normal)
!@Write:./path @/
>> multi-line content here
>> more content

// NaCommand (System)
!!@Shell:Watch -i abc123 -c 3

// Bash shortcut
!!@Bash: ls -la

// Pipeline
ls | grep Cargo | !@Write:./output.txt @/

// Async execution
echo hello @/Async(my_shell)

// Alias
ll    // → ls -la
gst   // → git status
```

## Execution Flow

```
User Input → alias expand → lexer → parser → RawCommands
                                                │
                         ┌──────────────────────┤
                         ↓                      ↓
                   Async (@/Async)         Sync Execution
                         │                      │
                   spawn_async ──→ pools    should_use_direct?
                                               │
                              ┌─────────────────┤
                              ↓                 ↓
                         Direct Mode       Captured Mode
                         (interactive)     (pipeline/format)
                              │                 │
                         Stdio::inherit    script -e -q -c
                         cd intercepted    dispatch() pipeline
```

## Project Structure

```
src/
├── main.rs              # Entry point, init, REPL launch
├── constants.rs         # All named constants
├── repl/
│   ├── mod.rs           # REPL loop, mode dispatch, broadcast, cleanup
│   ├── input.rs         # Multi-line input collection
│   ├── prompt.rs        # Prompt rendering with ANSI colors
│   └── signals.rs       # Signal handlers (SIGINT, SIGTERM, SIGHUP)
├── parser/
│   ├── mod.rs           # Parse entry: string → RawCommands
│   ├── lexer.rs         # Tokenizer: prefixes, pipes, quotes
│   ├── syntax.rs        # RawCommands, RawCmd, CmdType structs
│   ├── long_arg.rs      # @/ and blank-line long-argument extraction
│   └── pipeline.rs      # Pipe splitting (quote-safe)
├── executor/
│   ├── mod.rs           # Dispatch engine, safety check, build_nacommand
│   ├── shell_exec.rs    # exec_captured, exec_shell_direct, exec_bash, exec_cd
│   └── async_exec.rs    # spawn_async_shell_exec: full parse→dispatch in background
├── nacommand/
│   ├── mod.rs           # Execute NaCommand: builtin/config/plugin dispatch
│   ├── cmd.rs           # NaCommand, NaLevel structs
│   ├── registry.rs      # Command registry, lookup, help
│   ├── external.rs      # External config command execution (Phase 8)
│   └── builtin/
│       ├── write.rs     # Write command
│       ├── read.rs      # Read command (syntax highlighting)
│       ├── bash.rs      # Bash command (!!@Bash:)
│       ├── shell_cmd.rs # Shell management (!!@Shell:)
│       └── na_cmds.rs   # Command registry listing (!@NaCmds:)
├── shell/
│   ├── actor.rs         # Shell struct
│   ├── cmd.rs           # ShellCmd enum
│   ├── out.rs           # ShellOut enum
│   ├── pty.rs           # PTY session management
│   ├── cwd_sync.rs      # CWD synchronization
│   └── manager.rs       # ShellManager: main + async shells
├── plugin/
│   ├── mod.rs
│   ├── protocol.rs      # Message types: Call/Response/Off/Broadcast
│   ├── manifest.rs      # manifest.json loading and scanning
│   ├── manager.rs       # PluginManager: process lifecycle
│   ├── toexec.rs        # toExec recursion engine
│   └── broadcast.rs     # Broadcast event dispatch
├── config/
│   ├── mod.rs
│   ├── loader.rs        # KDL config loading
│   ├── schema.rs        # Config data structures
│   └── alias.rs         # Alias expansion
├── app/
│   ├── mod.rs           # AppData, CmdMeta, PluginMeta structs
│   └── init.rs          # Shell type detection
└── error/
    ├── mod.rs           # NashellError enum
    └── display.rs       # Error formatting
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `rustyline` | REPL line editing with history |
| `kdl-rs` | KDL configuration parsing |
| `serde` + `serde_json` | JSON serialization (config, plugins) |
| `syntect` | Syntax highlighting for Read command |
| `portable-pty` | PTY pseudo-terminal management |
| `libc` | Unix signal handling |
| `tokio` | Async runtime (future phases) |
| `log` + `env_logger` | Logging |
| `dirs` | System directory paths |

## Development

```bash
# Run all tests (330 tests)
cargo test

# Lint
cargo clippy

# Run with debug logging
RUST_LOG=debug cargo run

# Run with a specific config
NASHELL_CONFIG=/path/to/config.kdl cargo run
```

## Related Documentation

- [Development Design (CN)](docs/nashell_dev.md) — original design specification
- [Implementation Guide (CN)](docs/INSTRUCTION.md) — code style, architecture, data flow
- [Plugin Development Guide (CN)](docs/plugin_dev.md) — plugin protocol and examples
- [Implementation Phases (CN)](docs/phases.md) — phase-by-phase progress
