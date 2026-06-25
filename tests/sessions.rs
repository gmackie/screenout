use screenout::{
    parse_sessions, render_attach_info, render_session_list, resolve_attach_target, SessionInfo,
};

fn sample(name: &str, pane: &str, command: &str) -> SessionInfo {
    SessionInfo {
        name: name.to_string(),
        pane_id: pane.to_string(),
        command: command.to_string(),
    }
}

#[test]
fn parse_sessions_keeps_only_active_screenout_panes() {
    // fields: session window_active pane_active pane_id command
    let output = "\
screenout-htop 1 1 %3 htop
screenout-htop 0 1 %4 bash
other-session 1 1 %5 vim
screenout-build 1 1 %7 make
screenout-build 1 0 %8 less
";

    let sessions = parse_sessions(output);

    assert_eq!(
        sessions,
        vec![
            sample("screenout-htop", "%3", "htop"),
            sample("screenout-build", "%7", "make")
        ]
    );
}

#[test]
fn parse_sessions_ignores_malformed_lines() {
    assert_eq!(parse_sessions(""), vec![]);
    assert_eq!(parse_sessions("screenout-x 1 1\n"), vec![]);
}

#[test]
fn resolve_attach_target_matches_exact_or_prefixed_name() {
    let sessions = vec![sample("screenout-htop", "%3", "htop")];

    assert_eq!(
        resolve_attach_target(&sessions, Some("htop")).expect("prefixed"),
        sessions[0]
    );
    assert_eq!(
        resolve_attach_target(&sessions, Some("screenout-htop")).expect("exact"),
        sessions[0]
    );
}

#[test]
fn resolve_attach_target_uses_sole_session_when_unnamed() {
    let sessions = vec![sample("screenout-htop", "%3", "htop")];
    assert_eq!(
        resolve_attach_target(&sessions, None).expect("sole"),
        sessions[0]
    );
}

#[test]
fn resolve_attach_target_errors_on_no_match_and_ambiguity() {
    let sessions = vec![
        sample("screenout-htop", "%3", "htop"),
        sample("screenout-build", "%7", "make"),
    ];

    assert_eq!(
        resolve_attach_target(&sessions, Some("nope")).expect_err("no match"),
        "no screenout session matching 'nope'"
    );
    assert_eq!(
        resolve_attach_target(&sessions, None).expect_err("ambiguous"),
        "multiple screenout sessions: screenout-htop, screenout-build; pass a name to attach"
    );
    assert_eq!(
        resolve_attach_target(&[], None).expect_err("empty"),
        "no screenout sessions found"
    );
}

#[test]
fn render_session_list_shows_attach_and_agent_commands() {
    let sessions = vec![sample("screenout-htop", "%3", "htop")];

    assert_eq!(
        render_session_list(&sessions),
        "screenout: session screenout-htop (running htop)\n\
         screenout: attach command:\n\
         tmux attach-session -t screenout-htop\n\
         screenout: agent commands:\n\
         tmux capture-pane -p -t %3\n\
         tmux send-keys -t %3 'q' Enter\n"
    );
}

#[test]
fn render_session_list_handles_empty() {
    assert_eq!(
        render_session_list(&[]),
        "screenout: no screenout sessions found\n"
    );
}

#[test]
fn render_attach_info_shows_agent_commands() {
    let session = sample("screenout-htop", "%3", "htop");

    assert_eq!(
        render_attach_info(&session),
        "screenout: attaching to session screenout-htop\n\
         screenout: agent commands:\n\
         tmux capture-pane -p -t %3\n\
         tmux send-keys -t %3 'q' Enter\n"
    );
}
