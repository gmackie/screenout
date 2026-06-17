# Public Remote Rescue Design

## Goal

Make `screenout` ready for a first public release around one clear workflow:
rescue a command that is already running on a remote machine before the SSH
session disconnects.

The user flow is:

1. Start a long-running or interactive command over SSH.
2. Realize the SSH session may disconnect or that an agent should inspect it.
3. Press `Ctrl+Z` to stop the foreground process.
4. Run `screenout`.
5. Let `screenout` move the process into tmux, continue it, and print handoff
   commands.

The LLM use case comes from the same workflow: once the process is in tmux, an
agent can attach to the same session and diagnose the CLI application directly.

## Product Shape

`screenout` should remain a local command that operates on the machine where the
target process is running. It should not become an SSH orchestrator.

Its durable responsibilities are:

- find or accept the target process;
- create or reuse a tmux session on the current host;
- run `reptyr` for the target process;
- continue the target process;
- generate handoff commands for humans and agents.

There are two handoff modes:

- local attach command, always available;
- SSH attach command, available when the user supplies an SSH destination.

The local command is the source of truth:

```sh
tmux attach-session -t screenout-4242
```

The SSH command is a convenience wrapper:

```sh
ssh prod-box -t 'tmux attach-session -t screenout-4242'
```

## CLI Surface

The first public slice should support:

```sh
screenout
screenout --pid 4242
screenout --session build
screenout --ssh prod-box
screenout --ssh user@example.com
screenout --dry-run
```

`--ssh` means "also generate an SSH attach command using this destination." It
does not change where `screenout` runs, and it does not attempt to connect over
SSH. The value is used as the SSH destination so normal SSH aliases, usernames,
ports, and config file entries keep working.

If `--ssh` is supplied, the clipboard should receive the SSH handoff command.
Otherwise, it should receive the local tmux attach command. In both cases,
`screenout` should print the relevant commands so users are not dependent on
clipboard behavior.

Example output:

```text
screenout: moved PID 4242 into tmux session screenout-4242
screenout: copied attach command:
tmux attach-session -t screenout-4242
screenout: ssh handoff:
ssh prod-box -t 'tmux attach-session -t screenout-4242'
```

## Architecture

Keep process transfer and handoff rendering separate.

The execution plan should continue to model the commands that must run locally:

- `tmux new-session` or `tmux split-window`;
- `kill -CONT <pid>`;
- `tmux attach-session` when starting outside tmux.

Add a handoff layer that can render one or more commands from the selected tmux
session:

- local tmux attach;
- optional SSH attach.

This keeps SSH concerns out of process execution. It also makes dry-run output
and tests straightforward because handoff rendering is pure command formatting.

## Reliability And Verification

Automated tests should cover behavior that can be proven without a real
terminal transfer:

- command planning outside tmux;
- command planning inside tmux;
- automatic stopped-process selection;
- ambiguous target errors;
- local handoff rendering;
- SSH handoff rendering;
- shell quoting;
- clipboard command selection;
- dry-run output.

The real `reptyr` transfer should be verified with a manual smoke test on a
supported Unix host. The public docs should be clear that `reptyr` supports
Linux and FreeBSD, and that macOS is not a general process-transfer target.

Add a deterministic demo script under `examples/` for docs and manual testing.
The demo should start a visible long-running process that can be stopped with
`Ctrl+Z`, transferred with `screenout`, and observed after attaching to tmux.

## Public Documentation

The README should lead with the remote rescue workflow:

```sh
# On a remote box after starting a long-running command:
# press Ctrl+Z
screenout --ssh prod-box

# Later, from another terminal:
ssh prod-box -t 'tmux attach-session -t screenout-4242'
```

The docs should also explain the agent handoff:

```sh
tmux attach-session -t screenout-4242
```

Pass that command, or the SSH wrapper, to an agent that can interact with tmux.

## Non-Goals For The First Public Slice

- automatic SSH destination inference;
- SSH connection management;
- jump-host or bastion orchestration;
- pretending `reptyr` works everywhere;
- full release automation before the core workflow is proven.

