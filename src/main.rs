use screenout::{
    attach_command, build_plan, current_tmux_session_name, current_tty_name, detect_terminal_size,
    format_success_message, list_screenout_sessions, missing_dependencies, parse_args,
    read_current_tty_processes, render_attach_info, render_session_list, resolve_attach_target,
    run_plan, session_exists, CommandStep, Options, Subcommand, TermSize,
};
use std::process::Command;

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

    if let Some(subcommand) = args.subcommand {
        return run_subcommand(subcommand, args.dry_run);
    }

    let is_launch = args.command.is_some();
    let inside_tmux = std::env::var_os("TMUX").is_some();
    let current_tmux_session = if inside_tmux {
        current_tmux_session_name()
    } else {
        None
    };

    let needs_tty = !is_launch && args.pid.is_none();
    let current_tty = if needs_tty {
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
        let required: &[&str] = if is_launch {
            &["tmux"]
        } else {
            &["tmux", "reptyr", "kill"]
        };
        let missing = missing_dependencies(&path, required);
        if !missing.is_empty() {
            return Err(format!(
                "missing required command(s): {}",
                missing.join(", ")
            ));
        }

        if !inside_tmux && session_exists(&plan.tmux_session_name) {
            return Err(format!(
                "tmux session {} already exists; pass --session <name>",
                plan.tmux_session_name
            ));
        }
    }

    let pane_id = run_plan(&plan, args.dry_run)?;
    print!("{}", format_success_message(&plan, pane_id.as_deref()));
    Ok(())
}

fn run_subcommand(subcommand: Subcommand, dry_run: bool) -> Result<(), String> {
    let sessions = list_screenout_sessions();
    match subcommand {
        Subcommand::List => {
            print!("{}", render_session_list(&sessions));
            Ok(())
        }
        Subcommand::Attach(name) => {
            let target = resolve_attach_target(&sessions, name.as_deref())?;
            print!("{}", render_attach_info(&target));

            if dry_run {
                println!("{}", attach_command(&target.name));
                return Ok(());
            }

            let path = std::env::var("PATH").unwrap_or_default();
            let missing = missing_dependencies(&path, &["tmux"]);
            if !missing.is_empty() {
                return Err(format!(
                    "missing required command(s): {}",
                    missing.join(", ")
                ));
            }

            let step = CommandStep::new("tmux", ["attach-session", "-t", target.name.as_str()]);
            let status = Command::new(&step.program)
                .args(&step.args)
                .status()
                .map_err(|error| format!("failed to run tmux: {error}"))?;
            if !status.success() {
                return Err(format!("tmux exited with {status}"));
            }
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        "Usage:\n\
         \x20 screenout [--pid PID] [--session NAME] [--ssh DEST] [--size COLSxLINES] [--dry-run]\n\
         \x20 screenout [--session NAME] [--ssh DEST] [--size COLSxLINES] [--attach] [--dry-run] -- CMD [ARGS...]\n\n\
         Move a CLI tool (often a TUI) into tmux so a human and an agent can both drive it.\n\n\
         Rescue an already-running job:\n\
         \x20 1. Press Ctrl+Z to stop the foreground job.\n\
         \x20 2. Run screenout (or screenout --pid PID if several jobs are stopped).\n\n\
         Launch a fresh command into tmux (works on macOS; no reptyr):\n\
         \x20 screenout -- htop\n\
         \x20 screenout --attach -- top\n\n\
         Find and rejoin sessions later:\n\
         \x20 screenout list            list active screenout sessions\n\
         \x20 screenout attach [name]   reattach to a session\n\n\
         screenout prints a tmux attach command for humans and capture-pane/send-keys\n\
         commands (targeting the pane id) for agents. Use --ssh DEST to also print an\n\
         SSH attach wrapper. Detach from tmux with Ctrl+b then d."
    );
}
