# SSH Handoff Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add explicit SSH handoff support and public-demo documentation for rescuing remote CLI processes into tmux.

**Architecture:** Keep process execution local and add pure handoff rendering around the selected tmux session. `--ssh <destination>` only renders and copies an SSH wrapper; it never opens SSH itself.

**Tech Stack:** Rust 2021, standard library process execution, existing integration tests under `tests/`.

### Task 1: Model local and SSH handoff commands

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/planning.rs`

**Step 1: Write failing tests**

Add tests that assert a plan without SSH keeps the local handoff command and a plan with SSH renders both:

```rust
assert_eq!(plan.local_handoff_command, "tmux attach-session -t work");
assert_eq!(plan.clipboard_handoff_command, "ssh prod-box -t 'tmux attach-session -t work'");
assert_eq!(
    plan.ssh_handoff_command,
    Some("ssh prod-box -t 'tmux attach-session -t work'".to_string())
);
```

**Step 2: Run the focused tests**

Run: `cargo test --test planning handoff`

Expected: fail because `Options` and `Plan` do not have SSH handoff fields.

**Step 3: Implement minimal model changes**

Add `ssh_destination: Option<String>` to `Options`.

Replace `Plan::handoff_command` with:

- `local_handoff_command: String`
- `clipboard_handoff_command: String`
- `ssh_handoff_command: Option<String>`

Build the SSH command as:

```rust
ssh <destination> -t '<local tmux attach command>'
```

using existing shell quoting.

**Step 4: Update execution actions**

`CopyHandoff` should use `clipboard_handoff_command`.

**Step 5: Verify**

Run: `cargo test --test planning`

Expected: pass.

### Task 2: Parse `--ssh` and improve command output

**Files:**
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Test: add or update integration tests if output is factored into pure functions.

**Step 1: Write failing tests**

Add pure formatting tests for the completion output:

```rust
screenout: moved PID 4242 into tmux session work
screenout: attach command:
tmux attach-session -t work
screenout: ssh handoff:
ssh prod-box -t 'tmux attach-session -t work'
```

**Step 2: Implement parsing**

Accept:

```sh
screenout --ssh prod-box
```

and pass the value into `Options`.

**Step 3: Implement output helper**

Add a pure helper that renders user-facing success text from `Plan`, then print it after successful execution and in dry-run mode.

**Step 4: Verify**

Run: `cargo test`

Expected: pass.

### Task 3: Add deterministic demo and public docs

**Files:**
- Modify: `README.md`
- Create: `examples/remote-rescue-demo.sh`

**Step 1: Create demo script**

Add a POSIX shell script that prints its PID and ticks once per second until interrupted.

**Step 2: Update README**

Lead with the SSH rescue workflow:

```sh
./examples/remote-rescue-demo.sh
# press Ctrl+Z
screenout --ssh prod-box
```

Explain that `screenout` runs on the remote machine and that `--ssh` only formats the handoff command.

**Step 3: Verify demo script syntax**

Run: `sh -n examples/remote-rescue-demo.sh`

Expected: pass.

### Task 4: Full verification

**Files:**
- No source changes unless failures require fixes.

**Step 1: Format**

Run: `cargo fmt --check`

Expected: pass.

**Step 2: Test**

Run: `cargo test`

Expected: pass.

**Step 3: Build**

Run: `cargo build --release`

Expected: pass.

**Step 4: Commit**

Commit implementation and docs together:

```sh
git add src tests README.md examples docs/plans/2026-06-17-ssh-handoff-implementation.md
git commit -m "feat: add ssh handoff workflow"
```
