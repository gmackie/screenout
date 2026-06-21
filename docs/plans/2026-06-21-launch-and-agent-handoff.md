# Launch Mode And Agent Handoff Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `screenout -- <cmd>` launch mode, terminal-aware tmux sizing, and an agent-command handoff block, so CLI tools (especially TUIs) move into tmux in a state a human and an agent can both drive.

**Architecture:** Keep `build_plan` pure and the single source of truth. Extend `Options`/`Plan`, branch the plan on launch-vs-rescue, size sessions at creation with `-x/-y`, address agent commands by a runtime-captured `#{pane_id}`, and model the redraw nudge as a plain `kill -WINCH` step. Sizing detection and pane-id capture are the only impure additions and live in `main`/`run_plan`.

**Tech Stack:** Rust 2021, std only (no new crates — terminal size comes from `stty size` reading `/dev/tty`, not an ioctl crate). Integration tests under `tests/`.

Reference design: `docs/plans/2026-06-21-launch-and-agent-handoff-design.md`.

---

## Conventions

- TDD throughout: write the failing test, run it red, implement minimal code, run it green, commit.
- Run a single test with `cargo test --test <file> <name_substring>`.
- Run everything with `cargo test`.
- Keep `cargo fmt` clean and `cargo clippy --all-targets -- -D warnings` green; CI enforces both.

---

### Task 1: `TermSize` type and `--size` parsing

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/cli.rs`

**Step 1: Write the failing test**

Add to `tests/cli.rs`:

```rust
use screenout::{parse_size, TermSize};

#[test]
fn parses_valid_size() {
    assert_eq!(parse_size("160x48"), Ok(TermSize { cols: 160, lines: 48 }));
}

#[test]
fn rejects_malformed_size() {
    assert_eq!(parse_size("80"), Err("invalid --size value: 80 (expected COLSxLINES)".to_string()));
    assert_eq!(parse_size("0x40"), Err("invalid --size value: 0x40 (expected COLSxLINES)".to_string()));
    assert_eq!(parse_size("axb"), Err("invalid --size value: axb (expected COLSxLINES)".to_string()));
}
```

**Step 2: Run it red**

Run: `cargo test --test cli size`
Expected: FAIL — `TermSize`/`parse_size` undefined.

**Step 3: Implement minimal code** in `src/lib.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub lines: u16,
}

impl TermSize {
    pub const DEFAULT: TermSize = TermSize { cols: 120, lines: 40 };
}

pub fn parse_size(value: &str) -> Result<TermSize, String> {
    let invalid = || format!("invalid --size value: {value} (expected COLSxLINES)");
    let (cols, lines) = value.split_once('x').ok_or_else(invalid)?;
    let cols: u16 = cols.parse().map_err(|_| invalid())?;
    let lines: u16 = lines.parse().map_err(|_| invalid())?;
    if cols == 0 || lines == 0 {
        return Err(invalid());
    }
    Ok(TermSize { cols, lines })
}
```

**Step 4: Run it green**

Run: `cargo test --test cli size`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/cli.rs
git commit -m "feat: add TermSize and --size parsing"
```

---

### Task 2: Terminal size detection from `stty size`

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/cli.rs`

The pure parser is tested; the `stty` call itself is exercised by the manual smoke test.

**Step 1: Write the failing test** in `tests/cli.rs`:

```rust
use screenout::parse_stty_size;

#[test]
fn parses_stty_size_lines_then_cols() {
    // `stty size` prints "<lines> <cols>"
    assert_eq!(parse_stty_size("48 160\n"), Some(TermSize { cols: 160, lines: 48 }));
}

#[test]
fn rejects_empty_or_partial_stty_size() {
    assert_eq!(parse_stty_size(""), None);
    assert_eq!(parse_stty_size("48"), None);
    assert_eq!(parse_stty_size("0 0"), None);
}
```

**Step 2: Run it red**

Run: `cargo test --test cli stty`
Expected: FAIL — `parse_stty_size` undefined.

**Step 3: Implement** in `src/lib.rs`:

```rust
pub fn parse_stty_size(output: &str) -> Option<TermSize> {
    let mut parts = output.split_whitespace();
    let lines: u16 = parts.next()?.parse().ok()?;
    let cols: u16 = parts.next()?.parse().ok()?;
    if cols == 0 || lines == 0 {
        return None;
    }
    Some(TermSize { cols, lines })
}

/// Best-effort current terminal size via `stty size` reading from /dev/tty.
pub fn detect_terminal_size() -> Option<TermSize> {
    use std::fs::File;
    let tty = File::open("/dev/tty").ok()?;
    let output = Command::new("stty")
        .arg("size")
        .stdin(Stdio::from(tty))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_stty_size(&String::from_utf8_lossy(&output.stdout))
}
```

(`Command`, `Stdio` are already imported at the top of `lib.rs`.)

**Step 4: Run it green**

Run: `cargo test --test cli stty`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/cli.rs
git commit -m "feat: detect terminal size from stty"
```

---

### Task 3: Parse `--`, `--attach`, `--size` and reject contradictions

**Files:**
- Modify: `src/lib.rs` (`CliArgs`, `parse_args`)
- Test: `tests/cli.rs`

**Step 1: Write failing tests** in `tests/cli.rs`:

```rust
#[test]
fn parses_launch_command_after_double_dash() {
    let args = parse_args(["--session", "build", "--", "htop", "--delay", "2"]).expect("args");
    assert_eq!(args.session, Some("build".to_string()));
    assert_eq!(args.command, Some(vec!["htop".to_string(), "--delay".to_string(), "2".to_string()]));
}

#[test]
fn parses_attach_and_size_flags() {
    let args = parse_args(["--attach", "--size", "100x30", "--", "top"]).expect("args");
    assert!(args.attach);
    assert_eq!(args.size, Some(TermSize { cols: 100, lines: 30 }));
}

#[test]
fn rejects_empty_launch_command() {
    assert_eq!(parse_args(["--"]).expect_err("empty"), "-- requires a command to launch");
}

#[test]
fn rejects_pid_with_launch_command() {
    assert_eq!(
        parse_args(["--pid", "4242", "--", "htop"]).expect_err("contradiction"),
        "--pid cannot be combined with a launch command"
    );
}

#[test]
fn rejects_attach_without_launch_command() {
    assert_eq!(parse_args(["--attach"]).expect_err("no command"), "--attach requires a launch command");
}
```

The existing `parses_ssh_destination` / `rejects_missing_ssh_destination` tests must keep passing.

**Step 2: Run it red**

Run: `cargo test --test cli`
Expected: FAIL — `CliArgs` lacks `command`/`attach`/`size`.

**Step 3: Implement.** Extend `CliArgs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub pid: Option<u32>,
    pub session: Option<String>,
    pub ssh_destination: Option<String>,
    pub command: Option<Vec<String>>,
    pub attach: bool,
    pub size: Option<TermSize>,
    pub dry_run: bool,
    pub help: bool,
}
```

Initialize the three new fields (`command: None, attach: false, size: None`). Add match arms inside the `while let` loop, before the `unknown =>` arm:

```rust
"--attach" => parsed.attach = true,
"--size" => {
    let value = args.next().ok_or_else(|| "--size requires a value".to_string())?;
    parsed.size = Some(parse_size(value.as_ref())?);
}
"--" => {
    let rest: Vec<String> = args.by_ref().map(|a| a.as_ref().to_string()).collect();
    if rest.is_empty() {
        return Err("-- requires a command to launch".to_string());
    }
    parsed.command = Some(rest);
}
```

After the loop, before `Ok(parsed)`:

```rust
if parsed.command.is_some() && parsed.pid.is_some() {
    return Err("--pid cannot be combined with a launch command".to_string());
}
if parsed.attach && parsed.command.is_none() {
    return Err("--attach requires a launch command".to_string());
}
```

**Step 4: Run it green**

Run: `cargo test --test cli`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/cli.rs
git commit -m "feat: parse launch command, --attach, and --size"
```

---

### Task 4: Extend `Options`/`Plan` and rebuild the rescue plan (sizing, pane capture, WINCH, agent commands)

This task updates the rescue path only; launch comes in Task 6. Inside-tmux still uses `split-window` here and switches to `new-window` in Task 5 — keep changes scoped per task.

**Files:**
- Modify: `src/lib.rs` (`Options`, `Plan`, `build_plan`, `format_success_message`)
- Test: `tests/planning.rs`, `tests/cli.rs`

**Step 1: Write failing tests.** Replace the body of `explicit_pid_outside_tmux_creates_and_attaches_to_session` in `tests/planning.rs` so the plan carries size, pane capture, the WINCH step, and agent commands:

```rust
#[test]
fn explicit_pid_outside_tmux_creates_sized_session_with_pane_capture() {
    let options = Options {
        pid: Some(4242),
        session: Some("work".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: None,
        command: None,
        attach: false,
        size: TermSize { cols: 120, lines: 40 },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                [
                    "new-session", "-d", "-x", "120", "-y", "40",
                    "-P", "-F", "#{pane_id}", "-s", "work", "reptyr 4242",
                ]
            ),
            CommandStep::new("kill", ["-CONT", "4242"]),
            CommandStep::new("kill", ["-WINCH", "4242"]),
            CommandStep::new("tmux", ["attach-session", "-t", "work"]),
        ]
    );
    assert_eq!(plan.headline, "moved PID 4242 into tmux session work");
    assert_eq!(plan.local_handoff_command, "tmux attach-session -t work");
    assert_eq!(plan.clipboard_handoff_command, "tmux attach-session -t work");
    assert_eq!(plan.ssh_handoff_command, None);
    assert_eq!(plan.agent_capture_command, "tmux capture-pane -p -t {pane}");
    assert_eq!(plan.agent_send_keys_command, "tmux send-keys -t {pane} 'q' Enter");
}
```

Update every other `Options { .. }` literal in `tests/planning.rs` to add the three new fields (`command: None, attach: false, size: TermSize { cols: 120, lines: 40 }`), and update their expected `steps`/assertions to include `-x/-y/-P/-F` on the create step and the extra `kill -WINCH` step. Remove now-stale `plan.target_pid` assertions (the field is gone; assert `plan.headline` instead). In `tests/cli.rs`, update the two `Plan { .. }` literals to drop `target_pid` and add `headline`, `agent_capture_command`, `agent_send_keys_command`.

**Step 2: Run it red**

Run: `cargo test`
Expected: FAIL — `Options`/`Plan` field mismatch.

**Step 3: Implement.** Extend `Options`:

```rust
pub struct Options {
    pub pid: Option<u32>,
    pub session: Option<String>,
    pub inside_tmux: bool,
    pub current_tty: Option<String>,
    pub current_tmux_session: Option<String>,
    pub ssh_destination: Option<String>,
    pub command: Option<Vec<String>>,
    pub attach: bool,
    pub size: TermSize,
}
```

Replace `Plan`'s `target_pid: u32` with `headline: String` and add agent fields:

```rust
pub struct Plan {
    pub headline: String,
    pub tmux_session_name: String,
    pub local_handoff_command: String,
    pub ssh_handoff_command: Option<String>,
    pub clipboard_handoff_command: String,
    pub agent_capture_command: String,
    pub agent_send_keys_command: String,
    pub steps: Vec<CommandStep>,
}
```

Add `PlanError::EmptyCommand` with a `Display` arm (`"no command supplied to launch"`).

Rewrite `build_plan` for the rescue path (launch handled in Task 6 — for now keep `command` unused or `todo!`-free by branching to rescue when `command.is_none()` and returning `PlanError::EmptyCommand` otherwise so it compiles and tests pass):

```rust
pub fn build_plan(options: &Options, processes: &[ProcessRow]) -> Result<Plan, PlanError> {
    let TermSize { cols, lines } = options.size;

    // Resolve the inner command and a human subject for the headline.
    let (inner, subject): (String, String) = match &options.command {
        Some(_) => return Err(PlanError::EmptyCommand), // replaced in Task 6
        None => {
            let pid = match options.pid {
                Some(pid) => pid,
                None => choose_target(
                    processes,
                    options.current_tty.as_deref().unwrap_or(""),
                    std::process::id(),
                )?,
            };
            (format!("reptyr {pid}"), format!("PID {pid}"))
        }
    };
    let is_rescue = options.command.is_none();
    let continue_pid = options.pid; // refined in Task 6; for rescue this is the chosen pid

    // NOTE: choose_target may have picked the pid; capture it for the kill steps.
    let pid = parse_pid_from_inner(&inner); // helper below, rescue-only

    let cols_s = cols.to_string();
    let lines_s = lines.to_string();

    let (handoff_session, create_step, mut tail_steps) = if options.inside_tmux {
        let session = options
            .current_tmux_session
            .clone()
            .ok_or(PlanError::MissingTmuxSession)?;
        let create = CommandStep::new("tmux", ["split-window", inner.as_str()]); // -> new-window in Task 5
        (session, create, Vec::new())
    } else {
        let session = options
            .session
            .clone()
            .unwrap_or_else(|| format!("screenout-{pid}"));
        let create = CommandStep::new(
            "tmux",
            [
                "new-session", "-d", "-x", cols_s.as_str(), "-y", lines_s.as_str(),
                "-P", "-F", "#{pane_id}", "-s", session.as_str(), inner.as_str(),
            ],
        );
        (session, create, Vec::new())
    };

    let mut steps = vec![create_step];
    if is_rescue {
        let pid_s = pid.to_string();
        steps.push(CommandStep::new("kill", ["-CONT", pid_s.as_str()]));
        steps.push(CommandStep::new("kill", ["-WINCH", pid_s.as_str()]));
    }
    let should_attach = !options.inside_tmux && (is_rescue || options.attach);
    if should_attach {
        steps.push(CommandStep::new(
            "tmux",
            ["attach-session", "-t", handoff_session.as_str()],
        ));
    }
    steps.append(&mut tail_steps);

    let local_handoff_command = shell_words(&CommandStep::new(
        "tmux",
        ["attach-session", "-t", handoff_session.as_str()],
    ));
    let ssh_handoff_command = options.ssh_destination.as_ref().map(|destination| {
        shell_words(&CommandStep::new(
            "ssh",
            [destination.as_str(), "-t", local_handoff_command.as_str()],
        ))
    });
    let clipboard_handoff_command = ssh_handoff_command
        .clone()
        .unwrap_or_else(|| local_handoff_command.clone());

    let headline = if options.inside_tmux {
        format!("moved {subject} into a new tmux window (session {handoff_session})")
    } else {
        format!("moved {subject} into tmux session {handoff_session}")
    };

    let _ = (continue_pid,); // silence unused until Task 6 cleanup

    Ok(Plan {
        headline,
        tmux_session_name: handoff_session,
        local_handoff_command,
        ssh_handoff_command,
        clipboard_handoff_command,
        agent_capture_command: "tmux capture-pane -p -t {pane}".to_string(),
        agent_send_keys_command: "tmux send-keys -t {pane} 'q' Enter".to_string(),
        steps,
    })
}
```

> Implementer note: the `parse_pid_from_inner` / `continue_pid` scaffolding above is awkward because rescue needs the *chosen* pid (which may come from `choose_target`, not `options.pid`). Cleaner: have the `None` arm bind `let pid = ...;` in an outer `let` so both the inner string and the kill steps share it. Refactor to a single `pid` binding rather than re-parsing. Keep the public behavior identical to the test expectations.

Update `format_success_message` to use `headline` instead of `target_pid`/`tmux_session_name`:

```rust
pub fn format_success_message(plan: &Plan) -> String {
    let mut message = format!(
        "screenout: {}\n\
         screenout: attach command:\n\
         {}\n",
        plan.headline, plan.local_handoff_command
    );
    if let Some(ssh_handoff) = &plan.ssh_handoff_command {
        message.push_str("screenout: ssh handoff:\n");
        message.push_str(ssh_handoff);
        message.push('\n');
    }
    message
}
```

(The agent block is added to output in Task 7; `format_success_message`'s pane substitution also lands there. For now keep the two `tests/cli.rs` `format_success_message` tests asserting the `headline`-based first line.)

**Step 4: Run it green**

Run: `cargo test`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/planning.rs tests/cli.rs
git commit -m "feat: size sessions, capture pane, add WINCH and agent commands for rescue"
```

---

### Task 5: Inside-tmux uses `new-window` with pane capture

**Files:**
- Modify: `src/lib.rs` (`build_plan` inside-tmux branch)
- Test: `tests/planning.rs`

**Step 1: Update the failing test** `explicit_pid_inside_tmux_splits_current_session` → rename and rewrite to expect `new-window`:

```rust
#[test]
fn explicit_pid_inside_tmux_opens_new_window() {
    let options = Options {
        pid: Some(4242),
        session: None,
        inside_tmux: true,
        current_tty: None,
        current_tmux_session: Some("main".to_string()),
        ssh_destination: None,
        command: None,
        attach: false,
        size: TermSize { cols: 120, lines: 40 },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new("tmux", ["new-window", "-P", "-F", "#{pane_id}", "reptyr 4242"]),
            CommandStep::new("kill", ["-CONT", "4242"]),
            CommandStep::new("kill", ["-WINCH", "4242"]),
        ]
    );
    assert_eq!(plan.headline, "moved PID 4242 into a new tmux window (session main)");
    assert_eq!(plan.local_handoff_command, "tmux attach-session -t main");
}
```

**Step 2: Run it red**

Run: `cargo test --test planning new_window`
Expected: FAIL — still emits `split-window` without `-P -F`.

**Step 3: Implement.** In the inside-tmux branch of `build_plan`, replace the create step:

```rust
let create = CommandStep::new("tmux", ["new-window", "-P", "-F", "#{pane_id}", inner.as_str()]);
```

**Step 4: Run it green**

Run: `cargo test`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/planning.rs
git commit -m "feat: open inside-tmux processes in a sized new window"
```

---

### Task 6: Launch mode in `build_plan`

**Files:**
- Modify: `src/lib.rs` (`build_plan`, add `shell_join`, sanitize helper)
- Test: `tests/planning.rs`

**Step 1: Write failing tests** in `tests/planning.rs`:

```rust
#[test]
fn launch_outside_tmux_detached_creates_sized_session_without_kill() {
    let options = Options {
        pid: None,
        session: None,
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: None,
        command: Some(vec!["htop".to_string()]),
        attach: false,
        size: TermSize { cols: 100, lines: 30 },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![CommandStep::new(
            "tmux",
            [
                "new-session", "-d", "-x", "100", "-y", "30",
                "-P", "-F", "#{pane_id}", "-s", "screenout-htop", "htop",
            ]
        )]
    );
    assert_eq!(plan.headline, "launched htop in tmux session screenout-htop");
    assert_eq!(plan.local_handoff_command, "tmux attach-session -t screenout-htop");
}

#[test]
fn launch_with_attach_appends_attach_step() {
    let options = Options {
        pid: None,
        session: Some("mon".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: None,
        command: Some(vec!["top".to_string(), "-d".to_string(), "2".to_string()]),
        attach: true,
        size: TermSize { cols: 120, lines: 40 },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                [
                    "new-session", "-d", "-x", "120", "-y", "40",
                    "-P", "-F", "#{pane_id}", "-s", "mon", "top -d 2",
                ]
            ),
            CommandStep::new("tmux", ["attach-session", "-t", "mon"]),
        ]
    );
    assert_eq!(plan.headline, "launched top in tmux session mon");
}

#[test]
fn launch_quotes_command_arguments() {
    let options = Options {
        pid: None,
        session: Some("w".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: None,
        command: Some(vec!["sh".to_string(), "-c".to_string(), "echo hi".to_string()]),
        attach: false,
        size: TermSize { cols: 80, lines: 24 },
    };

    let plan = build_plan(&options, &[]).expect("plan");
    // the inner command is one shell-quoted string passed to tmux
    assert_eq!(plan.steps[0].args.last().unwrap(), "sh -c 'echo hi'");
}
```

**Step 2: Run it red**

Run: `cargo test --test planning launch`
Expected: FAIL — launch arm returns `PlanError::EmptyCommand`.

**Step 3: Implement.** Add helpers in `src/lib.rs`:

```rust
/// Join an argv into a single shell-safe command string for tmux to run.
pub fn shell_join(parts: &[String]) -> String {
    parts.iter().map(|p| shell_quote(p)).collect::<Vec<_>>().join(" ")
}

/// Derive a tmux-safe session name fragment from a command's program name.
fn session_fragment(program: &str) -> String {
    let base = program.rsplit('/').next().unwrap_or(program);
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() { "job".to_string() } else { trimmed.to_string() }
}
```

(`shell_quote` is currently private — `shell_join` lives in the same module so no visibility change is needed; keep `shell_quote` private.)

Replace the `match &options.command` block so the `Some` arm produces a real inner/subject and the default session name uses the fragment. Restructure so the chosen pid (rescue) is bound once:

```rust
let (inner, subject, rescue_pid): (String, String, Option<u32>) = match &options.command {
    Some(command) => {
        if command.is_empty() {
            return Err(PlanError::EmptyCommand);
        }
        (shell_join(command), session_fragment(&command[0]), None)
    }
    None => {
        let pid = match options.pid {
            Some(pid) => pid,
            None => choose_target(
                processes,
                options.current_tty.as_deref().unwrap_or(""),
                std::process::id(),
            )?,
        };
        (format!("reptyr {pid}"), format!("PID {pid}"), Some(pid))
    }
};
let is_rescue = rescue_pid.is_some();
```

Use `rescue_pid` for both the default session name (`format!("screenout-{pid}")` when present) and the kill steps. For launch, the default session name is `format!("screenout-{}", session_fragment(&command[0]))` — i.e. `subject`-derived; compute a `default_session` string accordingly:

```rust
let default_session = match rescue_pid {
    Some(pid) => format!("screenout-{pid}"),
    None => format!("screenout-{}", session_fragment(&options.command.as_ref().unwrap()[0])),
};
```

The kill steps are gated on `is_rescue` (already the case from Task 4) using `rescue_pid.unwrap()`. The headline:

```rust
let headline = match (&options.command, options.inside_tmux) {
    (Some(_), false) => format!("launched {subject} in tmux session {handoff_session}"),
    (Some(_), true)  => format!("launched {subject} in a new tmux window (session {handoff_session})"),
    (None, false)    => format!("moved {subject} into tmux session {handoff_session}"),
    (None, true)     => format!("moved {subject} into a new tmux window (session {handoff_session})"),
};
```

Delete the Task-4 `parse_pid_from_inner`/`continue_pid` scaffolding.

**Step 4: Run it green**

Run: `cargo test`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/planning.rs
git commit -m "feat: build launch-mode plans for fresh commands"
```

---

### Task 7: Capture pane id at execution and render the agent block

**Files:**
- Modify: `src/lib.rs` (`run_plan`, `format_success_message`, add `is_create_step`)
- Test: `tests/planning.rs` (execution actions unchanged), `tests/cli.rs` (message rendering)

**Step 1: Write failing tests** in `tests/cli.rs`:

```rust
#[test]
fn success_message_includes_agent_block_with_substituted_pane() {
    let plan = Plan {
        headline: "launched htop in tmux session w".to_string(),
        tmux_session_name: "w".to_string(),
        local_handoff_command: "tmux attach-session -t w".to_string(),
        ssh_handoff_command: None,
        clipboard_handoff_command: "tmux attach-session -t w".to_string(),
        agent_capture_command: "tmux capture-pane -p -t {pane}".to_string(),
        agent_send_keys_command: "tmux send-keys -t {pane} 'q' Enter".to_string(),
        steps: vec![],
    };

    let message = format_success_message(&plan, Some("%3"));

    assert_eq!(
        message,
        "screenout: launched htop in tmux session w\n\
         screenout: attach command:\n\
         tmux attach-session -t w\n\
         screenout: agent commands:\n\
         tmux capture-pane -p -t %3\n\
         tmux send-keys -t %3 'q' Enter\n"
    );
}

#[test]
fn success_message_keeps_placeholder_without_pane() {
    let plan = Plan {
        headline: "launched htop in tmux session w".to_string(),
        tmux_session_name: "w".to_string(),
        local_handoff_command: "tmux attach-session -t w".to_string(),
        ssh_handoff_command: Some("ssh box -t 'tmux attach-session -t w'".to_string()),
        clipboard_handoff_command: "ssh box -t 'tmux attach-session -t w'".to_string(),
        agent_capture_command: "tmux capture-pane -p -t {pane}".to_string(),
        agent_send_keys_command: "tmux send-keys -t {pane} 'q' Enter".to_string(),
        steps: vec![],
    };

    let message = format_success_message(&plan, None);
    assert!(message.contains("tmux capture-pane -p -t {pane}"));
    assert!(message.contains("screenout: ssh handoff:\n"));
}
```

Update the two existing `format_success_message` tests to call `format_success_message(&plan, None)` and to expect the new agent-commands block (place it after the attach command, before the ssh handoff). Their plans already gained the agent fields in Task 4.

**Step 2: Run it red**

Run: `cargo test --test cli`
Expected: FAIL — `format_success_message` takes one arg / no agent block.

**Step 3: Implement.** Change the signature and body:

```rust
pub fn format_success_message(plan: &Plan, pane: Option<&str>) -> String {
    let substitute = |s: &str| match pane {
        Some(pane) => s.replace("{pane}", pane),
        None => s.to_string(),
    };
    let mut message = format!(
        "screenout: {}\n\
         screenout: attach command:\n\
         {}\n\
         screenout: agent commands:\n\
         {}\n\
         {}\n",
        plan.headline,
        plan.local_handoff_command,
        substitute(&plan.agent_capture_command),
        substitute(&plan.agent_send_keys_command),
    );
    if let Some(ssh_handoff) = &plan.ssh_handoff_command {
        message.push_str("screenout: ssh handoff:\n");
        message.push_str(ssh_handoff);
        message.push('\n');
    }
    message
}
```

Add the create-step detector and capture stdout in `run_plan`, returning the pane id:

```rust
fn is_create_step(step: &CommandStep) -> bool {
    step.program == "tmux"
        && matches!(step.args.first().map(String::as_str), Some("new-session") | Some("new-window"))
}

pub fn run_plan(plan: &Plan, dry_run: bool) -> Result<Option<String>, String> {
    let path = std::env::var("PATH").unwrap_or_default();
    let mut pane_id: Option<String> = None;
    for action in build_execution_actions(plan) {
        match action {
            ExecutionAction::Run(step) => {
                if dry_run {
                    println!("{}", shell_words(&step));
                    continue;
                }
                if is_create_step(&step) {
                    let output = Command::new(&step.program)
                        .args(&step.args)
                        .output()
                        .map_err(|error| format!("failed to run {}: {error}", step.program))?;
                    if !output.status.success() {
                        return Err(format!("{} exited with {}", step.program, output.status));
                    }
                    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !id.is_empty() {
                        pane_id = Some(id);
                    }
                } else {
                    let status = Command::new(&step.program)
                        .args(&step.args)
                        .status()
                        .map_err(|error| format!("failed to run {}: {error}", step.program))?;
                    if !status.success() {
                        return Err(format!("{} exited with {status}", step.program));
                    }
                }
            }
            ExecutionAction::CopyHandoff(text) => {
                if dry_run {
                    println!("copy clipboard: {text}");
                    continue;
                }
                if !copy_handoff_to_clipboard(&text, &path)? {
                    eprintln!("screenout: clipboard command unavailable; handoff command:");
                    eprintln!("{text}");
                }
            }
        }
    }
    Ok(pane_id)
}
```

**Step 4: Run it green**

Run: `cargo test`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/cli.rs
git commit -m "feat: capture tmux pane id and render agent command block"
```

---

### Task 8: Session-collision guard

**Files:**
- Modify: `src/lib.rs` (add `session_exists`)
- Test: covered by manual smoke test (live tmux); no unit test for the `tmux has-session` call itself.

**Step 1: Implement** in `src/lib.rs`:

```rust
/// True if a tmux session with this name already exists.
pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
```

This is wired into `main` in Task 9. Commit with Task 9.

---

### Task 9: Wire `main.rs` — size resolution, mode dispatch, deps, collision, help

**Files:**
- Modify: `src/main.rs`
- Test: manual (`--help`, `--dry-run`).

**Step 1: Implement** `real_main`:

```rust
fn real_main() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1))?;
    if args.help {
        print_help();
        return Ok(());
    }

    let is_launch = args.command.is_some();
    let inside_tmux = std::env::var_os("TMUX").is_some();
    let current_tmux_session = if inside_tmux {
        current_tmux_session_name()
    } else {
        None
    };

    let needs_tty = !is_launch && args.pid.is_none();
    let current_tty = if needs_tty {
        Some(current_tty_name().ok_or_else(|| "could not determine current tty".to_string())?)
    } else {
        None
    };
    let processes = if let Some(tty) = &current_tty {
        read_current_tty_processes(tty).map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };

    let size = args
        .size
        .or_else(detect_terminal_size)
        .unwrap_or(TermSize::DEFAULT);

    let plan = build_plan(
        &Options {
            pid: args.pid,
            session: args.session,
            inside_tmux,
            current_tty,
            current_tmux_session,
            ssh_destination: args.ssh_destination,
            command: args.command,
            attach: args.attach,
            size,
        },
        &processes,
    )
    .map_err(|error| error.to_string())?;

    if !args.dry_run {
        let path = std::env::var("PATH").unwrap_or_default();
        let required: &[&str] = if is_launch {
            &["tmux"]
        } else {
            &["tmux", "reptyr", "kill"]
        };
        let missing = missing_dependencies(&path, required);
        if !missing.is_empty() {
            return Err(format!("missing required command(s): {}", missing.join(", ")));
        }

        if !inside_tmux && session_exists(&plan.tmux_session_name) {
            return Err(format!(
                "tmux session {} already exists; pass --session <name>",
                plan.tmux_session_name
            ));
        }
    }

    let pane_id = run_plan(&plan, args.dry_run)?;
    print!("{}", format_success_message(&plan, pane_id.as_deref()));
    Ok(())
}
```

Update the `use screenout::{...}` import list to add `detect_terminal_size`, `session_exists`, `TermSize`. Rewrite `print_help` to document both modes:

```rust
fn print_help() {
    println!(
        "Usage:\n\
         \x20 screenout [--pid PID] [--session NAME] [--ssh DEST] [--size COLSxLINES] [--dry-run]\n\
         \x20 screenout [--session NAME] [--ssh DEST] [--size COLSxLINES] [--attach] [--dry-run] -- CMD [ARGS...]\n\n\
         Move a CLI tool (often a TUI) into tmux so a human and an agent can both drive it.\n\n\
         Rescue an already-running job:\n\
         \x20 1. Press Ctrl+Z to stop the foreground job.\n\
         \x20 2. Run screenout (or screenout --pid PID if several jobs are stopped).\n\n\
         Launch a fresh command into tmux (works on macOS; no reptyr):\n\
         \x20 screenout -- htop\n\
         \x20 screenout --attach -- top\n\n\
         screenout prints a tmux attach command for humans and capture-pane/send-keys\n\
         commands (targeting the pane id) for agents. Use --ssh DEST to also print an\n\
         SSH attach wrapper. Detach from tmux with Ctrl+b then d."
    );
}
```

**Step 2: Verify manually**

```bash
cargo run -- --help
cargo run -- --dry-run -- htop
cargo run -- --pid 4242 --ssh prod-box --dry-run
```

Expected: help shows both modes; launch dry-run prints the `new-session ... -x -y ... -s screenout-htop htop` line and the agent block with the `{pane}` placeholder; rescue dry-run prints `new-session`, `kill -CONT`, `kill -WINCH`, `attach`, plus ssh handoff.

**Step 3: Commit**

```bash
git add src/lib.rs src/main.rs
git commit -m "feat: wire launch mode, sizing, and collision guard into the CLI"
```

---

### Task 10: Docs and demo

**Files:**
- Modify: `README.md`, `docs/release.md`
- Create: `examples/launch-demo.sh` (optional — a trivial TUI-ish loop)

**Step 1:** Update `README.md`:
- Add a "Launch mode" section near the top showing `screenout -- htop` and noting it works on macOS (no reptyr) and is the recommended way to hand a TUI to an agent.
- Document the agent-commands block (`capture-pane -p -t <pane>`, `send-keys -t <pane> ...`) and that agents should use those, not `attach-session`.
- Document `--attach` and `--size COLSxLINES`.
- Note the session-collision behavior (`--session` to pick another name).

**Step 2:** Update `docs/release.md` manual smoke test to add, on a Linux/FreeBSD host:
- launch: `target/release/screenout -- htop`, then `tmux capture-pane -p -t <pane>` shows a full-size htop, and `tmux send-keys -t <pane> q` quits it;
- rescue: confirm the moved TUI repaints at full size (WINCH) after attach;
- collision: running the same launch twice errors with the `--session` hint.
And on macOS: confirm launch mode works and rescue still reports `reptyr` missing.

**Step 3:** Verify any new script: `sh -n examples/launch-demo.sh` (if created).

**Step 4: Commit**

```bash
git add README.md docs/release.md examples
git commit -m "docs: document launch mode and agent handoff"
```

---

### Task 11: Full verification

**Files:** none unless failures require fixes.

**Steps:**

```bash
cargo fmt --check        # expected: clean
cargo clippy --all-targets -- -D warnings   # expected: no warnings
cargo test               # expected: all pass
cargo build --release    # expected: success
cargo package            # expected: success (matches CI)
```

Fix anything that fails, then a final commit if needed:

```bash
git add -A
git commit -m "chore: verification fixes for launch mode"
```

---

## Done When

- `screenout -- <cmd>` launches a sized, detached tmux session and prints attach + agent commands.
- `--attach` attaches; default does not.
- Rescue path sizes the session, sends `kill -WINCH`, and prints the agent block.
- Inside tmux, both paths open a full-size `new-window`.
- Agent commands address the real `#{pane_id}` after execution; dry-run shows `{pane}`.
- Re-launching a colliding session name errors with a `--session` hint.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo package` all pass.
