use screenout::{clipboard_command, CommandStep};

#[test]
fn selects_pbcopy_when_available() {
    let path = fake_path_with(&["pbcopy"]);

    let command = clipboard_command(&path).expect("clipboard command");

    assert_eq!(
        command,
        CommandStep::new("pbcopy", std::iter::empty::<&str>())
    );
}

#[test]
fn selects_wl_copy_before_xclip_when_available() {
    let path = fake_path_with(&["xclip", "wl-copy"]);

    let command = clipboard_command(&path).expect("clipboard command");

    assert_eq!(
        command,
        CommandStep::new("wl-copy", std::iter::empty::<&str>())
    );
}

#[test]
fn selects_xclip_with_clipboard_selection() {
    let path = fake_path_with(&["xclip"]);

    let command = clipboard_command(&path).expect("clipboard command");

    assert_eq!(
        command,
        CommandStep::new("xclip", ["-selection", "clipboard"])
    );
}

#[test]
fn returns_none_without_clipboard_backend() {
    assert_eq!(clipboard_command(""), None);
}

fn fake_path_with(names: &[&str]) -> String {
    let root = std::env::temp_dir().join(format!(
        "screenout-clipboard-{}-{}",
        std::process::id(),
        names.join("-")
    ));
    std::fs::create_dir_all(&root).expect("temp dir");

    for name in names {
        let file = root.join(name);
        std::fs::write(&file, "#!/bin/sh\n").expect("fake executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&file, permissions).expect("permissions");
        }
    }

    root.to_str().expect("utf8 path").to_string()
}
