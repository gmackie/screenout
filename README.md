# screenout

[![CI](https://github.com/gmackie/screenout/actions/workflows/ci.yml/badge.svg)](https://github.com/gmackie/screenout/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`screenout` moves a CLI tool — usually a TUI — into a tmux session so a human
and an agent can both interact with it.

It has two modes:

- **Launch** a fresh command into tmux. Works everywhere tmux runs, including
  macOS.
- **Rescue** an already-running job into tmux with `reptyr`, for the moment when
  you started something over SSH and realize the connection may drop or an agent
  needs to join.

In both modes `screenout` sizes the tmux window to your terminal, prints a tmux
attach command for humans, and prints `capture-pane`/`send-keys` commands for
agents.

## Launch mode

```sh
screenout -- htop
```

This creates a detached tmux session running `htop`, sized to your terminal, and
prints the handoff commands without blocking your shell. Because it does not use
`reptyr`, launch mode works on macOS.

Attach yourself immediately with `--attach`:

```sh
screenout --attach -- top
```

Everything after `--` is the command to run, passed through unchanged:

```sh
screenout -- tail -f /var/log/syslog
screenout --session build -- make -j8
```

## Rescue mode

```sh
# On the remote machine, while the command is running:
# press Ctrl+Z to stop the foreground job

screenout --ssh prod-box
```

When there is exactly one stopped process on the current terminal, `screenout`
creates a tmux session, starts `reptyr <pid>` inside it, sends `SIGCONT` (and a
`SIGWINCH` so the TUI repaints at the new size), copies a handoff command to your
clipboard, and attaches your terminal to tmux. If you are already inside tmux, it
opens a new full-size window instead.

`screenout` runs on the machine that owns the process. `--ssh` does not connect
anywhere; it only formats a command that another terminal or agent can use to
attach later.

If more than one stopped process is present, pass the target explicitly:

```sh
screenout --pid 4242
screenout --pid 4242 --session build
screenout --pid 4242 --ssh user@example.com
```

## Agent handoff

`screenout` always prints an **agent commands** block addressing the tmux pane by
its pane id. A headless agent cannot `attach` (that needs a real terminal), so it
drives the tool with these instead:

```text
screenout: agent commands:
tmux capture-pane -p -t %3      # read the current screen
tmux send-keys -t %3 'q' Enter  # send input
```

Pass these to an agent so it can read and drive the TUI. Humans use the printed
`tmux attach-session` command (or the `--ssh` wrapper).

With `--ssh prod-box`, the copied handoff command is:

```sh
ssh prod-box -t 'tmux attach-session -t screenout-4242'
```

## Sizing

By default `screenout` sizes the tmux window to your current terminal, falling
back to 120x40 when there is no terminal (for example when launched by an agent).
Override it explicitly:

```sh
screenout --size 160x48 -- htop
```

## Session names

Sessions are named `screenout-<pid>` (rescue) or `screenout-<command>` (launch).
If a session with that name already exists, `screenout` stops and asks you to
pick another with `--session`:

```sh
screenout --session htop-2 -- htop
```

## Dry run

Preview the commands without running them:

```sh
screenout --pid 4242 --ssh prod-box --dry-run
screenout --dry-run -- htop
```

Dry run also prints the clipboard text and shows the agent block with a `{pane}`
placeholder (the real pane id is only known once the session is created).

## Demo

Use the included demo script to practice the rescue flow:

```sh
./examples/remote-rescue-demo.sh
# press Ctrl+Z
screenout --ssh prod-box
```

Detach from tmux with `Ctrl+b` then `d`. Reattach from another terminal:

```sh
ssh prod-box -t 'tmux attach-session -t screenout-<pid>'
```

## Requirements

- `tmux`
- `reptyr` (rescue mode only — not needed for launch mode)
- Unix job control (rescue mode)

Clipboard copying is best effort. `screenout` looks for `pbcopy`, `wl-copy`,
`xclip`, or `xsel`. If none is available, it prints the handoff command instead.

`reptyr` supports Linux and FreeBSD. It is not a general macOS process-transfer
solution, so on macOS use launch mode.

On some Linux systems, ptrace restrictions may block `reptyr`; follow your
distribution's `reptyr` guidance before changing kernel security settings.

## Install

Prebuilt binaries for Linux (gnu and musl) and macOS (Intel and Apple Silicon)
are attached to each [release](https://github.com/gmackie/screenout/releases).
Download the archive for your platform, extract `screenout`, and put it on your
`PATH`:

```sh
tar -xzf screenout-x86_64-unknown-linux-gnu.tar.gz
sudo mv screenout /usr/local/bin/
```

Or build from source with Cargo:

```sh
cargo install --path .
```

## License

MIT
