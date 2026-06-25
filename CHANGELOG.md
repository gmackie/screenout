# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims
to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-25

### Added

- **Launch mode:** `screenout -- <cmd>` starts a fresh command inside a tmux
  session. It does not use `reptyr`, so it works on macOS. Detached by default;
  `--attach` attaches your terminal.
- **Terminal-aware sizing:** new tmux sessions are sized to the current terminal
  (detected via `stty size`), falling back to 120x40 when there is no terminal.
  `--size COLSxLINES` overrides detection.
- **Agent handoff block:** every run prints `tmux capture-pane`/`send-keys`
  commands addressing the pane by its captured `#{pane_id}`, so a headless agent
  can drive the tool without attaching a terminal.
- **Full-size windows inside tmux:** running inside tmux now opens a `new-window`
  instead of a cramped `split-window`.
- **Redraw nudge:** rescued processes receive `SIGWINCH` so TUIs repaint at the
  new size.
- **Session-collision guard:** `screenout` stops with a `--session` hint instead
  of failing when the target session name already exists.
- **`screenout list`** shows active screenout sessions with their attach and
  agent commands, so a backgrounded session is easy to find later.
- **`screenout attach [name]`** reattaches to a screenout session (the sole one
  when unnamed) and reprints its agent commands.

### Changed

- Rescued and launched sessions both surface attach commands for humans and
  agent commands for tools.

## [0.1.0]

### Added

- Initial release: rescue a stopped foreground job into tmux with `reptyr`, with
  local and `--ssh` handoff commands and best-effort clipboard copying.
