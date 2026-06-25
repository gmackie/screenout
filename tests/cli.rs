use screenout::{format_success_message, parse_args, parse_size, Plan, Subcommand, TermSize};

use screenout::parse_stty_size;

#[test]
fn parses_list_subcommand() {
    assert_eq!(
        parse_args(["list"]).expect("args").subcommand,
        Some(Subcommand::List)
    );
    assert_eq!(
        parse_args(["ls"]).expect("args").subcommand,
        Some(Subcommand::List)
    );
}

#[test]
fn parses_attach_subcommand_with_optional_name() {
    assert_eq!(
        parse_args(["attach"]).expect("args").subcommand,
        Some(Subcommand::Attach(None))
    );
    assert_eq!(
        parse_args(["attach", "build"]).expect("args").subcommand,
        Some(Subcommand::Attach(Some("build".to_string())))
    );
}

#[test]
fn attach_subcommand_accepts_dry_run() {
    let args = parse_args(["attach", "build", "--dry-run"]).expect("args");
    assert_eq!(
        args.subcommand,
        Some(Subcommand::Attach(Some("build".to_string())))
    );
    assert!(args.dry_run);
}

#[test]
fn attach_subcommand_rejects_extra_name() {
    assert_eq!(
        parse_args(["attach", "a", "b"]).expect_err("too many"),
        "attach takes at most one session name"
    );
}

#[test]
fn default_grammar_has_no_subcommand() {
    assert_eq!(parse_args(["--pid", "42"]).expect("args").subcommand, None);
    assert_eq!(parse_args(["--", "htop"]).expect("args").subcommand, None);
}

#[test]
fn parses_stty_size_lines_then_cols() {
    // `stty size` prints "<lines> <cols>"
    assert_eq!(
        parse_stty_size("48 160\n"),
        Some(TermSize {
            cols: 160,
            lines: 48
        })
    );
}

#[test]
fn rejects_empty_or_partial_stty_size() {
    assert_eq!(parse_stty_size(""), None);
    assert_eq!(parse_stty_size("48"), None);
    assert_eq!(parse_stty_size("0 0"), None);
}

#[test]
fn parses_valid_size() {
    assert_eq!(
        parse_size("160x48"),
        Ok(TermSize {
            cols: 160,
            lines: 48
        })
    );
}

#[test]
fn rejects_malformed_size() {
    assert_eq!(
        parse_size("80"),
        Err("invalid --size value: 80 (expected COLSxLINES)".to_string())
    );
    assert_eq!(
        parse_size("0x40"),
        Err("invalid --size value: 0x40 (expected COLSxLINES)".to_string())
    );
    assert_eq!(
        parse_size("axb"),
        Err("invalid --size value: axb (expected COLSxLINES)".to_string())
    );
}

#[test]
fn parses_ssh_destination() {
    let args = parse_args(["--ssh", "prod-box"]).expect("args");

    assert_eq!(args.ssh_destination, Some("prod-box".to_string()));
    assert_eq!(args.pid, None);
    assert_eq!(args.session, None);
    assert!(!args.dry_run);
}

#[test]
fn rejects_missing_ssh_destination() {
    let error = parse_args(["--ssh"]).expect_err("missing ssh destination");

    assert_eq!(error, "--ssh requires a destination");
}

#[test]
fn parses_launch_command_after_double_dash() {
    let args = parse_args(["--session", "build", "--", "htop", "--delay", "2"]).expect("args");
    assert_eq!(args.session, Some("build".to_string()));
    assert_eq!(
        args.command,
        Some(vec![
            "htop".to_string(),
            "--delay".to_string(),
            "2".to_string()
        ])
    );
}

#[test]
fn parses_attach_and_size_flags() {
    let args = parse_args(["--attach", "--size", "100x30", "--", "top"]).expect("args");
    assert!(args.attach);
    assert_eq!(
        args.size,
        Some(TermSize {
            cols: 100,
            lines: 30
        })
    );
}

#[test]
fn rejects_empty_launch_command() {
    assert_eq!(
        parse_args(["--"]).expect_err("empty"),
        "-- requires a command to launch"
    );
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
    assert_eq!(
        parse_args(["--attach"]).expect_err("no command"),
        "--attach requires a launch command"
    );
}

#[test]
fn formats_success_message_with_local_handoff_only() {
    let plan = Plan {
        headline: "moved PID 4242 into tmux session work".to_string(),
        tmux_session_name: "work".to_string(),
        local_handoff_command: "tmux attach-session -t work".to_string(),
        ssh_handoff_command: None,
        clipboard_handoff_command: "tmux attach-session -t work".to_string(),
        agent_capture_command: "tmux capture-pane -p -t {pane}".to_string(),
        agent_send_keys_command: "tmux send-keys -t {pane} 'q' Enter".to_string(),
        steps: vec![],
    };

    assert_eq!(
        format_success_message(&plan, None),
        "screenout: moved PID 4242 into tmux session work\n\
         screenout: attach command:\n\
         tmux attach-session -t work\n\
         screenout: agent commands:\n\
         tmux capture-pane -p -t {pane}\n\
         tmux send-keys -t {pane} 'q' Enter\n"
    );
}

#[test]
fn formats_success_message_with_ssh_handoff() {
    let plan = Plan {
        headline: "moved PID 4242 into tmux session work".to_string(),
        tmux_session_name: "work".to_string(),
        local_handoff_command: "tmux attach-session -t work".to_string(),
        ssh_handoff_command: Some("ssh prod-box -t 'tmux attach-session -t work'".to_string()),
        clipboard_handoff_command: "ssh prod-box -t 'tmux attach-session -t work'".to_string(),
        agent_capture_command: "tmux capture-pane -p -t {pane}".to_string(),
        agent_send_keys_command: "tmux send-keys -t {pane} 'q' Enter".to_string(),
        steps: vec![],
    };

    assert_eq!(
        format_success_message(&plan, None),
        "screenout: moved PID 4242 into tmux session work\n\
         screenout: attach command:\n\
         tmux attach-session -t work\n\
         screenout: agent commands:\n\
         tmux capture-pane -p -t {pane}\n\
         tmux send-keys -t {pane} 'q' Enter\n\
         screenout: ssh handoff:\n\
         ssh prod-box -t 'tmux attach-session -t work'\n"
    );
}

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
