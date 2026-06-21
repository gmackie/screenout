use screenout::{
    build_plan, current_tmux_session_name, current_tty_name, detect_terminal_size,
    format_success_message, missing_dependencies, parse_args, read_current_tty_processes, run_plan,
    Options, TermSize,
};

fn main() {
    if let Err(error) = real_main() {
        eprintln!("screenout: {error}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1))?;
    if args.help {
        print_help();
        return Ok(());
    }

    let inside_tmux = std::env::var_os("TMUX").is_some();
    let current_tmux_session = if inside_tmux {
        current_tmux_session_name()
    } else {
        None
    };
    let current_tty = if args.pid.is_none() {
        Some(current_tty_name().ok_or_else(|| "could not determine current tty".to_string())?)
    } else {
        None
    };
    let processes = if let Some(tty) = &current_tty {
        read_current_tty_processes(tty).map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let size = args
        .size
        .or_else(detect_terminal_size)
        .unwrap_or(TermSize::DEFAULT);
    let plan = build_plan(
        &Options {
            pid: args.pid,
            session: args.session,
            inside_tmux,
            current_tty,
            current_tmux_session,
            ssh_destination: args.ssh_destination,
            command: args.command,
            attach: args.attach,
            size,
        },
        &processes,
    )
    .map_err(|error| error.to_string())?;

    if !args.dry_run {
        let path = std::env::var("PATH").unwrap_or_default();
        let missing = missing_dependencies(&path, &["tmux", "reptyr", "kill"]);
        if !missing.is_empty() {
            return Err(format!(
                "missing required command(s): {}",
                missing.join(", ")
            ));
        }
    }

    let pane_id = run_plan(&plan, args.dry_run)?;
    print!("{}", format_success_message(&plan, pane_id.as_deref()));
    Ok(())
}

fn print_help() {
    println!(
        "Usage: screenout [--pid PID] [--session NAME] [--ssh DESTINATION] [--dry-run]\n\n\
         Move a stopped foreground job into tmux using reptyr.\n\n\
         Workflow:\n\
           1. Press Ctrl+Z in the terminal running the job.\n\
           2. Run screenout, or screenout --pid PID if more than one job is stopped.\n\
           3. Use --ssh DESTINATION to also print an SSH attach command.\n\
           4. Share the copied tmux attach command with an agent.\n\
           5. Detach from tmux later with Ctrl+b then d."
    );
}
