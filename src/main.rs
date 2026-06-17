use screenout::{
    build_plan, current_tmux_session_name, current_tty_name, missing_dependencies,
    read_current_tty_processes, run_plan, Options,
};

fn main() {
    if let Err(error) = real_main() {
        eprintln!("screenout: {error}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let mut pid = None;
    let mut session = None;
    let mut dry_run = false;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pid" | "-p" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--pid requires a process id".to_string())?;
                pid = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid process id: {value}"))?,
                );
            }
            "--session" | "-s" => {
                session = Some(
                    args.next()
                        .ok_or_else(|| "--session requires a name".to_string())?,
                );
            }
            "--dry-run" => dry_run = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }

    let inside_tmux = std::env::var_os("TMUX").is_some();
    let current_tmux_session = if inside_tmux {
        current_tmux_session_name()
    } else {
        None
    };
    let current_tty = if pid.is_none() {
        Some(current_tty_name().ok_or_else(|| "could not determine current tty".to_string())?)
    } else {
        None
    };
    let processes = if let Some(tty) = &current_tty {
        read_current_tty_processes(tty).map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let plan = build_plan(
        &Options {
            pid,
            session,
            inside_tmux,
            current_tty,
            current_tmux_session,
        },
        &processes,
    )
    .map_err(|error| error.to_string())?;

    if !dry_run {
        let path = std::env::var("PATH").unwrap_or_default();
        let missing = missing_dependencies(&path, &["tmux", "reptyr", "kill"]);
        if !missing.is_empty() {
            return Err(format!(
                "missing required command(s): {}",
                missing.join(", ")
            ));
        }
    }

    run_plan(&plan, dry_run)
}

fn print_help() {
    println!(
        "Usage: screenout [--pid PID] [--session NAME] [--dry-run]\n\n\
         Move a stopped foreground job into tmux using reptyr.\n\n\
         Workflow:\n\
           1. Press Ctrl+Z in the terminal running the job.\n\
           2. Run screenout, or screenout --pid PID if more than one job is stopped.\n\
           3. Share the copied tmux attach command with an agent.\n\
           4. Detach from tmux later with Ctrl+b then d."
    );
}
