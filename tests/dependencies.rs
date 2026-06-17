use std::fs;

use screenout::missing_dependencies;

#[test]
fn reports_tmux_and_reptyr_missing_from_empty_path() {
    let missing = missing_dependencies("", &["tmux", "reptyr"]);

    assert_eq!(missing, vec!["tmux".to_string(), "reptyr".to_string()]);
}

#[test]
fn finds_executable_dependencies_on_path() {
    let temp = std::env::temp_dir().join(format!("screenout-deps-{}", std::process::id()));
    fs::create_dir_all(&temp).expect("temp dir");
    let tmux = temp.join("tmux");
    fs::write(&tmux, "#!/bin/sh\n").expect("tmux");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&tmux).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&tmux, permissions).expect("permissions");
    }

    let missing = missing_dependencies(temp.to_str().expect("utf8 path"), &["tmux", "reptyr"]);

    fs::remove_dir_all(&temp).ok();
    assert_eq!(missing, vec!["reptyr".to_string()]);
}
