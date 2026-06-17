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

Run this on a Linux or FreeBSD host with `tmux` and `reptyr` installed:

```sh
./examples/remote-rescue-demo.sh
# press Ctrl+Z
target/release/screenout --ssh <host-or-ssh-alias>
```

Confirm:

- the demo process resumes inside tmux;
- the local attach command is printed;
- the SSH handoff command is printed and copied when `--ssh` is used;
- detaching with `Ctrl+b` then `d` leaves the process running;
- reattaching with the printed command returns to the same tmux session.

## Publish

Only publish after the automated checks and manual smoke test pass.

```sh
cargo publish --dry-run
cargo publish
```
