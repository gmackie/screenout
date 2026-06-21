# Launch Mode And Agent Handoff Design

## Goal

Make `screenout` good at its real purpose: moving CLI tools (usually TUIs) into a
tmux session so a human and an agent can both interact with them.

The existing rescue path (Ctrl+Z a job, adopt it with `reptyr`) optimizes for "a
human reconnects later." Three gaps block the agent/TUI use case:

1. **TUI sizing.** A detached tmux session defaults to 80x24, so moved or
   launched TUIs render cramped and a non-attached agent's `capture-pane` only
   ever sees 80x24.
2. **Agent interactivity.** The only handoff emitted is `tmux attach-session`,
   which needs a real TTY. Headless agents drive tmux with `send-keys` and
   `capture-pane` against a `pane` target, which `screenout` never surfaces.
3. **Launch.** Rescue requires an already-running, stopped job and `reptyr`,
   which does not work on macOS. Launching a tool directly into tmux sidesteps
   both.

## Modes And CLI Surface

`screenout` has two modes sharing one plan/handoff core. Mode is determined from
args: presence of a `--` terminator means launch; absence means rescue.

- **Rescue** (existing): `screenout [--pid N]` — adopt a Ctrl+Z'd job via
  `reptyr`.
- **Launch** (new): `screenout -- <cmd> [args...]` — start a fresh command in
  tmux. No `reptyr`; works on macOS. Everything after `--` is the target
  command, unparsed.

```sh
screenout                         # rescue: auto-detect stopped job
screenout --pid 4242              # rescue: explicit pid
screenout -- htop                 # launch: detached (default)
screenout --attach -- htop        # launch + attach your terminal
screenout --session build -- top  # name the session
screenout --size 160x48 -- htop   # override sizing
screenout --ssh prod-box          # ssh handoff wrapper (both modes)
screenout --dry-run -- htop       # preview commands
```

Flags `--pid`, `--session`, `--ssh`, `--dry-run`, `--help` are unchanged. New:

- `--attach` — launch only; attach your terminal after creating the session.
- `--size COLSxLINES` — override the detected terminal size.

## Plan Model

The pure `build_plan` core stays the source of truth and is extended, not forked.

`Options` gains:

- `command: Option<Vec<String>>` — `Some(argv)` in launch mode, `None` in rescue.
- `attach: bool`
- `size: TermSize` — concrete `(cols, lines)`, resolved before `build_plan`.

The inner command run inside tmux is `reptyr <pid>` (rescue) or the quoted user
command (launch).

## Sizing

Size is resolved in `main` (impure) and passed into `build_plan` as a concrete
`TermSize`, keeping the plan deterministic and testable. Precedence:

1. `--size COLSxLINES` if given;
2. else current terminal via `TIOCGWINSZ` (`ioctl` on stdout/stderr/`/dev/tty`);
3. else fallback `120x40`.

Applied at creation:

- Outside tmux (launch or rescue): `tmux new-session -d -x <cols> -y <lines>
  -s <name> <inner>`.
- Inside tmux: `tmux new-window` — a full-size window inheriting the attached
  client's dimensions, so `-x/-y` does not apply there.

## Pane Targeting

Agent commands address a stable `pane_id` (e.g. `%3`); humans attach by session
name. The create step adds `-P -F '#{pane_id}'` and the executor captures its
stdout. Because `build_plan` is pure and cannot know the runtime pane id, the
plan carries a `{pane}` placeholder in its agent-command strings; the executor
substitutes the real `pane_id` after the create step runs.

## Execution Flow

The create step uses `Command::output` (not `status`) to capture the pane id,
then a `{pane}` -> real-id substitution runs before printing.

Per-mode ordering:

- Launch, detached: create (capture pane) -> print handoff.
- Launch, `--attach`: create -> copy clipboard -> `tmux attach-session`.
- Rescue outside tmux: `new-session` (capture pane) -> `kill -CONT <pid>` ->
  attach.
- Rescue inside tmux: `new-window` (capture pane) -> `kill -CONT <pid>`. No
  attach; switch to the new window.

After the create step, rescue sends the inner process `SIGWINCH` so the TUI
repaints at the new size. Launch starts fresh at the right size and relies on
tmux's own sizing.

## Output Block

Always printed; dry-run shows it with the `{pane}` placeholder.

```text
screenout: launched htop in tmux session screenout-htop (window 2)
screenout: attach command:
tmux attach-session -t screenout-htop
screenout: agent commands:
tmux capture-pane -p -t %3
tmux send-keys -t %3 'q' Enter
screenout: ssh handoff:        # only with --ssh
ssh prod-box -t 'tmux attach-session -t screenout-htop'
```

## Error Handling

- Session-name collision: check `tmux has-session -t <name>` before
  `new-session`; on conflict, error and suggest `--session <other>`. Inside-tmux
  `new-window` does not collide.
- Launch + `--pid`: rejected at parse time.
- Empty launch command (`screenout --` with nothing after): error.
- `--size` parse: require `<cols>x<lines>`, both positive ints; else error.
- No tty for size detection: silently fall back to `120x40`.
- macOS rescue: unchanged; `reptyr` dependency check still fires. Launch is the
  macOS-friendly path, documented as such.

## Testing

Pure functions, no real terminal:

- launch plan outside tmux (`new-session` with `-x/-y`, `-P -F`, quoted command);
- launch plan inside tmux (`new-window`, no `-x/-y`);
- launch `--attach` adds attach step; detached does not;
- rescue plans updated for `-x/-y` and pane capture;
- size resolution precedence (flag > detected > default), detection injected;
- `--size` parsing valid/invalid;
- arg parsing: `--` terminator, launch + `--pid` rejection, empty command;
- agent-command rendering with `{pane}` placeholder and substitution;
- pane-id substitution helper.

SIGWINCH, real pane capture, and `tmux has-session` need a live tmux and are
covered by the manual smoke test in `docs/release.md`.

## Non-Goals For This Slice

- JSON / `--agent` structured output (defer until a real consumer needs it).
- `--split` opt-in for inside-tmux (new-window is the default and only behavior).
- SIGWINCH for launched processes (tmux owns their sizing).
- Automatic SSH connection management (unchanged from the original design).
