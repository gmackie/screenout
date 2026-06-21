use std::fmt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub lines: u16,
}

impl TermSize {
    pub const DEFAULT: TermSize = TermSize {
        cols: 120,
        lines: 40,
    };
}

pub fn parse_size(value: &str) -> Result<TermSize, String> {
    let invalid = || format!("invalid --size value: {value} (expected COLSxLINES)");
    let (cols, lines) = value.split_once('x').ok_or_else(invalid)?;
    let cols: u16 = cols.parse().map_err(|_| invalid())?;
    let lines: u16 = lines.parse().map_err(|_| invalid())?;
    if cols == 0 || lines == 0 {
        return Err(invalid());
    }
    Ok(TermSize { cols, lines })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub pid: Option<u32>,
    pub session: Option<String>,
    pub inside_tmux: bool,
    pub current_tty: Option<String>,
    pub current_tmux_session: Option<String>,
    pub ssh_destination: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub pid: Option<u32>,
    pub session: Option<String>,
    pub ssh_destination: Option<String>,
    pub dry_run: bool,
    pub help: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub target_pid: u32,
    pub tmux_session_name: String,
    pub local_handoff_command: String,
    pub ssh_handoff_command: Option<String>,
    pub clipboard_handoff_command: String,
    pub steps: Vec<CommandStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStep {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionAction {
    Run(CommandStep),
    CopyHandoff(String),
}

impl CommandStep {
    pub fn new<I, S>(program: impl Into<String>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    pub stat: String,
    pub tty: String,
    pub command: String,
}

impl ProcessRow {
    pub fn new(pid: u32, ppid: u32, stat: &str, tty: &str, command: &str) -> Self {
        Self {
            pid,
            ppid,
            stat: stat.to_string(),
            tty: tty.to_string(),
            command: command.to_string(),
        }
    }
}

pub fn parse_args<I, S>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut parsed = CliArgs {
        pid: None,
        session: None,
        ssh_destination: None,
        dry_run: false,
        help: false,
    };
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_ref() {
            "--pid" | "-p" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--pid requires a process id".to_string())?;
                parsed.pid = Some(
                    value
                        .as_ref()
                        .parse()
                        .map_err(|_| format!("invalid process id: {}", value.as_ref()))?,
                );
            }
            "--session" | "-s" => {
                parsed.session = Some(
                    args.next()
                        .ok_or_else(|| "--session requires a name".to_string())?
                        .as_ref()
                        .to_string(),
                );
            }
            "--ssh" => {
                parsed.ssh_destination = Some(
                    args.next()
                        .ok_or_else(|| "--ssh requires a destination".to_string())?
                        .as_ref()
                        .to_string(),
                );
            }
            "--dry-run" => parsed.dry_run = true,
            "--help" | "-h" => parsed.help = true,
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }

    Ok(parsed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    NoTarget,
    AmbiguousTargets(Vec<u32>),
    InvalidPsRow(String),
    MissingTmuxSession,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::NoTarget => write!(
                f,
                "no stopped process found on the current terminal; pass --pid <pid>"
            ),
            PlanError::AmbiguousTargets(pids) => {
                write!(f, "multiple stopped processes found: ")?;
                for (index, pid) in pids.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{pid}")?;
                }
                write!(f, "; pass --pid <pid>")
            }
            PlanError::InvalidPsRow(row) => write!(f, "could not parse ps row: {row}"),
            PlanError::MissingTmuxSession => write!(
                f,
                "could not determine current tmux session for handoff command"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

pub fn build_plan(options: &Options, processes: &[ProcessRow]) -> Result<Plan, PlanError> {
    let target_pid = match options.pid {
        Some(pid) => pid,
        None => choose_target(
            processes,
            options.current_tty.as_deref().unwrap_or(""),
            std::process::id(),
        )?,
    };

    let reptyr = format!("reptyr {target_pid}");
    let continue_target = target_pid.to_string();
    let (handoff_session, steps) = if options.inside_tmux {
        let handoff_session = options
            .current_tmux_session
            .clone()
            .ok_or(PlanError::MissingTmuxSession)?;
        let steps = vec![
            CommandStep::new("tmux", ["split-window", reptyr.as_str()]),
            CommandStep::new("kill", ["-CONT", continue_target.as_str()]),
        ];
        (handoff_session, steps)
    } else {
        let session = options
            .session
            .clone()
            .unwrap_or_else(|| format!("screenout-{target_pid}"));
        let steps = vec![
            CommandStep::new(
                "tmux",
                ["new-session", "-d", "-s", session.as_str(), reptyr.as_str()],
            ),
            CommandStep::new("kill", ["-CONT", continue_target.as_str()]),
            CommandStep::new("tmux", ["attach-session", "-t", session.as_str()]),
        ];
        (session, steps)
    };
    let local_handoff_command = shell_words(&CommandStep::new(
        "tmux",
        ["attach-session", "-t", handoff_session.as_str()],
    ));
    let ssh_handoff_command = options.ssh_destination.as_ref().map(|destination| {
        shell_words(&CommandStep::new(
            "ssh",
            [destination.as_str(), "-t", local_handoff_command.as_str()],
        ))
    });
    let clipboard_handoff_command = ssh_handoff_command
        .clone()
        .unwrap_or_else(|| local_handoff_command.clone());

    Ok(Plan {
        target_pid,
        tmux_session_name: handoff_session,
        local_handoff_command,
        ssh_handoff_command,
        clipboard_handoff_command,
        steps,
    })
}

pub fn format_success_message(plan: &Plan) -> String {
    let mut message = format!(
        "screenout: moved PID {} into tmux session {}\n\
         screenout: attach command:\n\
         {}\n",
        plan.target_pid, plan.tmux_session_name, plan.local_handoff_command
    );

    if let Some(ssh_handoff) = &plan.ssh_handoff_command {
        message.push_str("screenout: ssh handoff:\n");
        message.push_str(ssh_handoff);
        message.push('\n');
    }

    message
}

pub fn choose_target(
    processes: &[ProcessRow],
    current_tty: &str,
    current_pid: u32,
) -> Result<u32, PlanError> {
    let targets: Vec<u32> = processes
        .iter()
        .filter(|process| process.tty == current_tty)
        .filter(|process| process.pid != current_pid)
        .filter(|process| process.stat.contains('T'))
        .map(|process| process.pid)
        .collect();

    match targets.as_slice() {
        [pid] => Ok(*pid),
        [] => Err(PlanError::NoTarget),
        _ => Err(PlanError::AmbiguousTargets(targets)),
    }
}

pub fn parse_ps_rows(output: &str) -> Result<Vec<ProcessRow>, PlanError> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("PID"))
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                return Err(PlanError::InvalidPsRow(line.to_string()));
            }
            let pid = fields[0]
                .parse()
                .map_err(|_| PlanError::InvalidPsRow(line.to_string()))?;
            let ppid = fields[1]
                .parse()
                .map_err(|_| PlanError::InvalidPsRow(line.to_string()))?;
            Ok(ProcessRow::new(
                pid,
                ppid,
                fields[2],
                fields[3],
                &fields[4..].join(" "),
            ))
        })
        .collect()
}

pub fn current_tty_name() -> Option<String> {
    let output = Command::new("tty").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    tty.rsplit('/').next().map(str::to_string)
}

pub fn current_tmux_session_name() -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|session| !session.is_empty())
}

pub fn read_current_tty_processes(tty: &str) -> Result<Vec<ProcessRow>, PlanError> {
    let output = Command::new("ps")
        .args(["-o", "pid=,ppid=,stat=,tty=,comm=", "-t", tty])
        .output()
        .map_err(|error| PlanError::InvalidPsRow(error.to_string()))?;
    parse_ps_rows(&String::from_utf8_lossy(&output.stdout))
}

pub fn run_plan(plan: &Plan, dry_run: bool) -> Result<(), String> {
    let path = std::env::var("PATH").unwrap_or_default();
    for action in build_execution_actions(plan) {
        match action {
            ExecutionAction::Run(step) => {
                if dry_run {
                    println!("{}", shell_words(&step));
                    continue;
                }

                let status = Command::new(&step.program)
                    .args(&step.args)
                    .status()
                    .map_err(|error| format!("failed to run {}: {error}", step.program))?;
                if !status.success() {
                    return Err(format!("{} exited with {status}", step.program));
                }
            }
            ExecutionAction::CopyHandoff(text) => {
                if dry_run {
                    println!("copy clipboard: {text}");
                    continue;
                }
                if !copy_handoff_to_clipboard(&text, &path)? {
                    eprintln!("screenout: clipboard command unavailable; handoff command:");
                    eprintln!("{text}");
                }
            }
        }
    }
    Ok(())
}

pub fn build_execution_actions(plan: &Plan) -> Vec<ExecutionAction> {
    let mut actions = Vec::new();
    let mut copied = false;

    for step in &plan.steps {
        if is_tmux_attach_step(step) && !copied {
            actions.push(ExecutionAction::CopyHandoff(
                plan.clipboard_handoff_command.clone(),
            ));
            copied = true;
        }
        actions.push(ExecutionAction::Run(step.clone()));
    }

    if !copied {
        actions.push(ExecutionAction::CopyHandoff(
            plan.clipboard_handoff_command.clone(),
        ));
    }

    actions
}

fn is_tmux_attach_step(step: &CommandStep) -> bool {
    step.program == "tmux" && step.args.first().map(String::as_str) == Some("attach-session")
}

pub fn copy_handoff_to_clipboard(text: &str, path: &str) -> Result<bool, String> {
    let Some(command) = clipboard_command(path) else {
        return Ok(false);
    };
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to run {}: {error}", command.program))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| format!("failed to open stdin for {}", command.program))?
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write clipboard text: {error}"))?;

    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {}: {error}", command.program))?;
    if !status.success() {
        return Err(format!("{} exited with {status}", command.program));
    }

    Ok(true)
}

pub fn clipboard_command(path: &str) -> Option<CommandStep> {
    if path_contains_executable(path, "pbcopy") {
        return Some(CommandStep::new("pbcopy", std::iter::empty::<&str>()));
    }
    if path_contains_executable(path, "wl-copy") {
        return Some(CommandStep::new("wl-copy", std::iter::empty::<&str>()));
    }
    if path_contains_executable(path, "xclip") {
        return Some(CommandStep::new("xclip", ["-selection", "clipboard"]));
    }
    if path_contains_executable(path, "xsel") {
        return Some(CommandStep::new("xsel", ["--clipboard", "--input"]));
    }
    None
}

pub fn missing_dependencies(path: &str, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .copied()
        .filter(|name| !path_contains_executable(path, name))
        .map(str::to_string)
        .collect()
}

pub fn shell_words(step: &CommandStep) -> String {
    std::iter::once(step.program.as_str())
        .chain(step.args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=@".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn path_contains_executable(path: &str, name: &str) -> bool {
    std::env::split_paths(path).any(|directory| is_executable(&directory.join(name)))
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}
