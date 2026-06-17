# screenout

`screenout` rescues a stopped terminal job into a tmux session.

It is built for the moment when you started a long-running or interactive
command over SSH, then realized your connection may drop or an agent needs to
join the session.

```sh
# On the remote machine, while the command is running:
# press Ctrl+Z to stop the foreground job

screenout --ssh prod-box
```

When there is exactly one stopped process on the current terminal, `screenout`
creates a tmux session, starts `reptyr <pid>` inside it, sends `SIGCONT` to the
process, copies a handoff command to your clipboard, and attaches your terminal
to tmux. If you are already inside tmux, it opens a new pane and copies an
attach command for the current tmux session.

`screenout` runs on the machine that owns the process. `--ssh` does not connect
anywhere; it only formats a command that another terminal or agent can use to
attach later.

With `--ssh prod-box`, the copied handoff command is:

```sh
ssh prod-box -t 'tmux attach-session -t screenout-4242'
```

The local attach command is always printed too:

```sh
tmux attach-session -t screenout-4242
```

Pass that command to an agent so it can join the same tmux session.

If more than one stopped process is present, pass the target explicitly:

```sh
screenout --pid 4242
screenout --pid 4242 --session build
screenout --pid 4242 --ssh user@example.com
```

Preview the commands without running them:

```sh
screenout --pid 4242 --ssh prod-box --dry-run
```

Dry run also prints the clipboard text that would be copied.

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
- `reptyr`
- Unix job control

Clipboard copying is best effort. `screenout` looks for `pbcopy`, `wl-copy`,
`xclip`, or `xsel`. If none is available, it prints the handoff command instead.

`reptyr` supports Linux and FreeBSD. It is not a general macOS process-transfer
solution.

On some Linux systems, ptrace restrictions may block `reptyr`; follow your
distribution's `reptyr` guidance before changing kernel security settings.

## Install

```sh
cargo install --path .
```
