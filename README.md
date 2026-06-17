# screenout

`screenout` moves a stopped terminal job into a tmux session.

The intended workflow is:

```sh
# In the terminal running the long-lived command:
# press Ctrl+Z

screenout
```

When there is exactly one stopped process on the current terminal, `screenout`
creates a tmux session, starts `reptyr <pid>` inside it, sends `SIGCONT` to the
process, copies a tmux attach command to your clipboard, and attaches your
terminal to tmux. If you are already inside tmux, it opens a new pane and copies
an attach command for the current tmux session.

The copied command is the handoff point for interactive co-working:

```sh
tmux attach-session -t screenout-4242
```

Pass that command to an agent so it can join the same tmux session.

If more than one stopped process is present, pass the target explicitly:

```sh
screenout --pid 4242
screenout --pid 4242 --session build
```

Preview the commands without running them:

```sh
screenout --pid 4242 --dry-run
```

Dry run also prints the clipboard text that would be copied.

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
