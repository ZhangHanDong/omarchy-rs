use std::{env, path::PathBuf};

fn main() -> Result<(), String> {
    let _force = env::args().any(|arg| arg == "--force");
    let codex_home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .ok_or("HOME and CODEX_HOME are unset")?;
    let record = omarchy_agents::codex::collect_local_record(&codex_home.join("sessions"))?;
    println!(
        "{}",
        serde_json::to_string(&record).map_err(|error| error.to_string())?
    );
    Ok(())
}
