use std::{env, path::PathBuf, process::ExitCode};

use omarchy_cli::{Command, Layout};

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("omarchy-rs: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let command = Command::parse(&args)?;
    let layout = Layout::from_environment()?;
    let path = env::var_os("PATH").unwrap_or_default();
    let source = env::var_os("OMARCHY_RS_RELEASE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            env::current_exe()
                .ok()?
                .parent()
                .map(|path| path.to_path_buf())
        })
        .ok_or("cannot locate release sibling directory")?;
    omarchy_cli::execute(command, &layout, &source, &path)
}
