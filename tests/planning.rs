use screenout::{
    build_execution_actions, build_plan, choose_target, parse_ps_rows, CommandStep,
    ExecutionAction, Options, PlanError, ProcessRow,
};

#[test]
fn explicit_pid_outside_tmux_creates_and_attaches_to_session() {
    let options = Options {
        pid: Some(4242),
        session: Some("work".to_string()),
        inside_tmux: false,
        current_tty: None,
        current_tmux_session: None,
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new("tmux", ["new-session", "-d", "-s", "work", "reptyr 4242"]),
            CommandStep::new("kill", ["-CONT", "4242"]),
            CommandStep::new("tmux", ["attach-session", "-t", "work"]),
        ]
    );
    assert_eq!(plan.handoff_command, "tmux attach-session -t work");
    assert_eq!(
        build_execution_actions(&plan),
        vec![
            ExecutionAction::Run(CommandStep::new(
                "tmux",
                ["new-session", "-d", "-s", "work", "reptyr 4242"]
            )),
            ExecutionAction::Run(CommandStep::new("kill", ["-CONT", "4242"])),
            ExecutionAction::CopyHandoff("tmux attach-session -t work".to_string()),
            ExecutionAction::Run(CommandStep::new("tmux", ["attach-session", "-t", "work"])),
        ]
    );
}

#[test]
fn explicit_pid_inside_tmux_splits_current_session() {
    let options = Options {
        pid: Some(4242),
        session: None,
        inside_tmux: true,
        current_tty: None,
        current_tmux_session: Some("main".to_string()),
    };

    let plan = build_plan(&options, &[]).expect("plan");

    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new("tmux", ["split-window", "reptyr 4242"]),
            CommandStep::new("kill", ["-CONT", "4242"]),
        ]
    );
    assert_eq!(plan.handoff_command, "tmux attach-session -t main");
    assert_eq!(
        build_execution_actions(&plan),
        vec![
            ExecutionAction::Run(CommandStep::new("tmux", ["split-window", "reptyr 4242"])),
            ExecutionAction::Run(CommandStep::new("kill", ["-CONT", "4242"])),
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
    };

    let error = build_plan(&options, &[]).expect_err("missing tmux session");

    assert_eq!(error, PlanError::MissingTmuxSession);
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
    };
    let rows = vec![
        ProcessRow::new(1111, 1000, "T", "ttys003", "vim"),
        ProcessRow::new(2222, 1000, "T", "ttys004", "less"),
    ];

    let plan = build_plan(&options, &rows).expect("plan");

    assert_eq!(plan.target_pid, 1111);
    assert_eq!(
        plan.steps,
        vec![
            CommandStep::new(
                "tmux",
                ["new-session", "-d", "-s", "screenout-1111", "reptyr 1111"]
            ),
            CommandStep::new("kill", ["-CONT", "1111"]),
            CommandStep::new("tmux", ["attach-session", "-t", "screenout-1111"]),
        ]
    );
    assert_eq!(
        plan.handoff_command,
        "tmux attach-session -t screenout-1111"
    );
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
