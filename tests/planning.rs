use screenout::{
    build_execution_actions, build_plan, choose_target, parse_ps_rows, CommandStep,
    ExecutionAction, Options, PlanError, ProcessRow, TermSize,
};

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
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                [
                    "new-session",
                    "-d",
                    "-x",
                    "120",
                    "-y",
                    "40",
                    "-P",
                    "-F",
                    "#{pane_id}",
                    "-s",
                    "work",
                    "reptyr 4242",
                ]
            ),
            CommandStep::new("kill", ["-CONT", "4242"]),
            CommandStep::new("kill", ["-WINCH", "4242"]),
            CommandStep::new("tmux", ["attach-session", "-t", "work"]),
        ]
    );
    assert_eq!(plan.headline, "moved PID 4242 into tmux session work");
    assert_eq!(plan.local_handoff_command, "tmux attach-session -t work");
    assert_eq!(
        plan.clipboard_handoff_command,
        "tmux attach-session -t work"
    );
    assert_eq!(plan.ssh_handoff_command, None);
    assert_eq!(plan.agent_capture_command, "tmux capture-pane -p -t {pane}");
    assert_eq!(
        plan.agent_send_keys_command,
        "tmux send-keys -t {pane} 'q' Enter"
    );
    assert_eq!(
        build_execution_actions(&plan),
        vec![
            ExecutionAction::Run(CommandStep::new(
                "tmux",
                [
                    "new-session",
                    "-d",
                    "-x",
                    "120",
                    "-y",
                    "40",
                    "-P",
                    "-F",
                    "#{pane_id}",
                    "-s",
                    "work",
                    "reptyr 4242",
                ]
            )),
            ExecutionAction::Run(CommandStep::new("kill", ["-CONT", "4242"])),
            ExecutionAction::Run(CommandStep::new("kill", ["-WINCH", "4242"])),
            ExecutionAction::CopyHandoff("tmux attach-session -t work".to_string()),
            ExecutionAction::Run(CommandStep::new("tmux", ["attach-session", "-t", "work"])),
        ]
    );
}

#[test]
fn explicit_pid_with_ssh_destination_builds_ssh_handoff() {
    let options = Options {
        pid: Some(4242),
        session: Some("work".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: Some("prod-box".to_string()),
        command: None,
        attach: false,
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(plan.local_handoff_command, "tmux attach-session -t work");
    assert_eq!(
        plan.ssh_handoff_command,
        Some("ssh prod-box -t 'tmux attach-session -t work'".to_string())
    );
    assert_eq!(
        plan.clipboard_handoff_command,
        "ssh prod-box -t 'tmux attach-session -t work'"
    );
    assert_eq!(
        build_execution_actions(&plan),
        vec![
            ExecutionAction::Run(CommandStep::new(
                "tmux",
                [
                    "new-session",
                    "-d",
                    "-x",
                    "120",
                    "-y",
                    "40",
                    "-P",
                    "-F",
                    "#{pane_id}",
                    "-s",
                    "work",
                    "reptyr 4242",
                ]
            )),
            ExecutionAction::Run(CommandStep::new("kill", ["-CONT", "4242"])),
            ExecutionAction::Run(CommandStep::new("kill", ["-WINCH", "4242"])),
            ExecutionAction::CopyHandoff(
                "ssh prod-box -t 'tmux attach-session -t work'".to_string()
            ),
            ExecutionAction::Run(CommandStep::new("tmux", ["attach-session", "-t", "work"])),
        ]
    );
}

#[test]
fn ssh_handoff_quotes_destination_and_nested_attach_command() {
    let options = Options {
        pid: Some(4242),
        session: Some("build job".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: Some("user@example.com".to_string()),
        command: None,
        attach: false,
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.local_handoff_command,
        "tmux attach-session -t 'build job'"
    );
    assert_eq!(
        plan.ssh_handoff_command,
        Some("ssh user@example.com -t 'tmux attach-session -t '\\''build job'\\'''".to_string())
    );
}

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
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                ["new-window", "-P", "-F", "#{pane_id}", "reptyr 4242"]
            ),
            CommandStep::new("kill", ["-CONT", "4242"]),
            CommandStep::new("kill", ["-WINCH", "4242"]),
        ]
    );
    assert_eq!(
        plan.headline,
        "moved PID 4242 into a new tmux window (session main)"
    );
    assert_eq!(plan.local_handoff_command, "tmux attach-session -t main");
    assert_eq!(
        plan.clipboard_handoff_command,
        "tmux attach-session -t main"
    );
    assert_eq!(plan.ssh_handoff_command, None);
    assert_eq!(
        build_execution_actions(&plan),
        vec![
            ExecutionAction::Run(CommandStep::new(
                "tmux",
                ["new-window", "-P", "-F", "#{pane_id}", "reptyr 4242"]
            )),
            ExecutionAction::Run(CommandStep::new("kill", ["-CONT", "4242"])),
            ExecutionAction::Run(CommandStep::new("kill", ["-WINCH", "4242"])),
            ExecutionAction::CopyHandoff("tmux attach-session -t main".to_string()),
        ]
    );
}

#[test]
fn inside_tmux_requires_current_session_for_handoff() {
    let options = Options {
        pid: Some(4242),
        session: None,
        inside_tmux: true,
        current_tty: None,
        current_tmux_session: None,
        ssh_destination: None,
        command: None,
        attach: false,
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let error = build_plan(&options, &[]).expect_err("missing tmux session");

    assert_eq!(error, PlanError::MissingTmuxSession);
}

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
        size: TermSize {
            cols: 100,
            lines: 30,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![CommandStep::new(
            "tmux",
            [
                "new-session",
                "-d",
                "-x",
                "100",
                "-y",
                "30",
                "-P",
                "-F",
                "#{pane_id}",
                "-s",
                "screenout-htop",
                "htop",
            ]
        )]
    );
    assert_eq!(
        plan.headline,
        "launched htop in tmux session screenout-htop"
    );
    assert_eq!(
        plan.local_handoff_command,
        "tmux attach-session -t screenout-htop"
    );
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
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                [
                    "new-session",
                    "-d",
                    "-x",
                    "120",
                    "-y",
                    "40",
                    "-P",
                    "-F",
                    "#{pane_id}",
                    "-s",
                    "mon",
                    "top -d 2",
                ]
            ),
            CommandStep::new("tmux", ["attach-session", "-t", "mon"]),
        ]
    );
    assert_eq!(plan.headline, "launched top in tmux session mon");
}

#[test]
fn launch_inside_tmux_opens_new_window_without_attach() {
    let options = Options {
        pid: None,
        session: None,
        inside_tmux: true,
        current_tty: None,
        current_tmux_session: Some("main".to_string()),
        ssh_destination: None,
        command: Some(vec!["htop".to_string()]),
        attach: false,
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![CommandStep::new(
            "tmux",
            ["new-window", "-P", "-F", "#{pane_id}", "htop"]
        )]
    );
    assert_eq!(
        plan.headline,
        "launched htop in a new tmux window (session main)"
    );
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
        command: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo hi".to_string(),
        ]),
        attach: false,
        size: TermSize {
            cols: 80,
            lines: 24,
        },
    };

    let plan = build_plan(&options, &[]).expect("plan");
    // the inner command is one shell-quoted string passed to tmux
    assert_eq!(plan.steps[0].args.last().unwrap(), "sh -c 'echo hi'");
}

#[test]
fn stopped_process_parser_reads_ps_rows() {
    let rows = parse_ps_rows(
        r#"
          PID    PPID STAT TTY      COMM
         1111   1000 T    ttys003  vim
         2222   1000 S+   ttys003  zsh
        "#,
    )
    .expect("rows");

    assert_eq!(
        rows,
        vec![
            ProcessRow {
                pid: 1111,
                ppid: 1000,
                stat: "T".to_string(),
                tty: "ttys003".to_string(),
                command: "vim".to_string(),
            },
            ProcessRow {
                pid: 2222,
                ppid: 1000,
                stat: "S+".to_string(),
                tty: "ttys003".to_string(),
                command: "zsh".to_string(),
            },
        ]
    );
}

#[test]
fn chooses_single_stopped_process_on_current_tty() {
    let rows = vec![
        ProcessRow::new(1111, 1000, "T", "ttys003", "vim"),
        ProcessRow::new(2222, 1000, "S+", "ttys003", "zsh"),
        ProcessRow::new(3333, 1000, "T", "ttys004", "less"),
    ];

    let pid = choose_target(&rows, "ttys003", 9999).expect("target");

    assert_eq!(pid, 1111);
}

#[test]
fn automatic_pid_uses_supplied_current_tty() {
    let options = Options {
        pid: None,
        session: None,
        inside_tmux: false,
        current_tty: Some("ttys003".to_string()),
        current_tmux_session: None,
        ssh_destination: None,
        command: None,
        attach: false,
        size: TermSize {
            cols: 120,
            lines: 40,
        },
    };
    let rows = vec![
        ProcessRow::new(1111, 1000, "T", "ttys003", "vim"),
        ProcessRow::new(2222, 1000, "T", "ttys004", "less"),
    ];

    let plan = build_plan(&options, &rows).expect("plan");

    assert_eq!(
        plan.headline,
        "moved PID 1111 into tmux session screenout-1111"
    );
    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                [
                    "new-session",
                    "-d",
                    "-x",
                    "120",
                    "-y",
                    "40",
                    "-P",
                    "-F",
                    "#{pane_id}",
                    "-s",
                    "screenout-1111",
                    "reptyr 1111",
                ]
            ),
            CommandStep::new("kill", ["-CONT", "1111"]),
            CommandStep::new("kill", ["-WINCH", "1111"]),
            CommandStep::new("tmux", ["attach-session", "-t", "screenout-1111"]),
        ]
    );
    assert_eq!(
        plan.local_handoff_command,
        "tmux attach-session -t screenout-1111"
    );
    assert_eq!(
        plan.clipboard_handoff_command,
        "tmux attach-session -t screenout-1111"
    );
    assert_eq!(plan.ssh_handoff_command, None);
}

#[test]
fn refuses_ambiguous_stopped_processes() {
    let rows = vec![
        ProcessRow::new(1111, 1000, "T", "ttys003", "vim"),
        ProcessRow::new(2222, 1000, "T", "ttys003", "less"),
    ];

    let error = choose_target(&rows, "ttys003", 9999).expect_err("ambiguous");

    assert_eq!(error, PlanError::AmbiguousTargets(vec![1111, 2222]));
}
