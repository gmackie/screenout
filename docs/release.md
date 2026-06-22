# Release Checklist

Use this checklist before publishing a public `screenout` release.

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
cargo package
```

## Manual Smoke Test

### Launch mode (any host with tmux, including macOS)

```sh
target/release/screenout -- htop
```

Confirm:

- a detached tmux session is created and the command keeps running;
- the printed `tmux capture-pane -p -t <pane>` shows a full-size `htop` (not
  cramped to 80x24);
- `tmux send-keys -t <pane> q` quits it;
- running the same launch twice errors with the `--session` hint (collision);
- `--attach -- top` attaches your terminal instead of staying detached;
- on macOS, launch mode works and rescue mode still reports `reptyr` missing.

### Rescue mode (Linux or FreeBSD with `tmux` and `reptyr`)

```sh
./examples/remote-rescue-demo.sh
# press Ctrl+Z
target/release/screenout --ssh <host-or-ssh-alias>
```

Confirm:

- the demo process resumes inside tmux and repaints at full size (the `SIGWINCH`
  nudge);
- the local attach command and the agent commands block are printed;
- the SSH handoff command is printed and copied when `--ssh` is used;
- detaching with `Ctrl+b` then `d` leaves the process running;
- reattaching with the printed command returns to the same tmux session.

## Publish

Only publish after the automated checks and manual smoke test pass.

```sh
cargo publish --dry-run
cargo publish
```
