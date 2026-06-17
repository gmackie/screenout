use screenout::{format_success_message, parse_args, Plan};

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
