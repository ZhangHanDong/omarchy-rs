use std::{env, path::PathBuf, process::ExitCode};

use omarchy_rs::{Command, Layout};

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
    if args.first().map(String::as_str) == Some("cleaner") {
        return omarchy_rs::cleaner::execute_cli(&args[1..]);
    }
    let command = Command::parse(&args)?;
    if command == Command::Version {
        return Ok(format!("omarchy-rs {}", env!("CARGO_PKG_VERSION")));
    }
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
    omarchy_rs::execute(command, &layout, &source, &path)
}
