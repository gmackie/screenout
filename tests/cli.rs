use screenout::{format_success_message, parse_args, parse_size, Plan, TermSize};

use screenout::parse_stty_size;

#[test]
fn parses_stty_size_lines_then_cols() {
    // `stty size` prints "<lines> <cols>"
    assert_eq!(
        parse_stty_size("48 160\n"),
        Some(TermSize { cols: 160, lines: 48 })
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
    assert_eq!(parse_size("160x48"), Ok(TermSize { cols: 160, lines: 48 }));
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
fn formats_success_message_with_local_handoff_only() {
    let plan = Plan {
        target_pid: 4242,
        tmux_session_name: "work".to_string(),
        local_handoff_command: "tmux attach-session -t work".to_string(),
        ssh_handoff_command: None,
        clipboard_handoff_command: "tmux attach-session -t work".to_string(),
        steps: vec![],
    };

    assert_eq!(
        format_success_message(&plan),
        "screenout: moved PID 4242 into tmux session work\n\
         screenout: attach command:\n\
         tmux attach-session -t work\n"
    );
}

#[test]
fn formats_success_message_with_ssh_handoff() {
    let plan = Plan {
        target_pid: 4242,
        tmux_session_name: "work".to_string(),
        local_handoff_command: "tmux attach-session -t work".to_string(),
        ssh_handoff_command: Some("ssh prod-box -t 'tmux attach-session -t work'".to_string()),
        clipboard_handoff_command: "ssh prod-box -t 'tmux attach-session -t work'".to_string(),
        steps: vec![],
    };

    assert_eq!(
        format_success_message(&plan),
        "screenout: moved PID 4242 into tmux session work\n\
         screenout: attach command:\n\
         tmux attach-session -t work\n\
         screenout: ssh handoff:\n\
         ssh prod-box -t 'tmux attach-session -t work'\n"
    );
}
