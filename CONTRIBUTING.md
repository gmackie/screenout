# Contributing to screenout

Thanks for your interest in improving `screenout`. It is a small, dependency-free
Rust CLI, so the contribution loop is quick.

## Development

```sh
git clone https://github.com/gmackie/screenout
cd screenout
cargo build
cargo test
```

Before opening a pull request, run the same checks CI runs:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo package
```

## Design principles

- **`build_plan` is pure and is the source of truth.** It turns options into a
  list of command steps and handoff strings without running anything. Keep
  process execution and terminal/tmux IO in thin wrappers (`run_plan`, the
  helpers in `main.rs`) so the planning logic stays unit-testable.
- **No new dependencies without a strong reason.** The crate is std-only on
  purpose; terminal size comes from `stty`, not an ioctl crate.
- **Test-driven.** Add a failing test first, then the minimal code to pass it.
  Most behavior can be proven without a real terminal — assert on the planned
  command steps and rendered strings.

## Things that need a real host

`reptyr` transfers and live tmux behavior (pane capture, `has-session`,
`SIGWINCH` redraw) cannot be unit-tested. The manual smoke test in
`docs/release.md` covers them on a Linux or FreeBSD host. `reptyr` does not work
on macOS; use launch mode there.

## Pull requests

- Keep changes focused and include tests.
- Update `README.md`, `CHANGELOG.md`, and `docs/` when behavior changes.
- Describe how you verified the change (commands run, output observed).
