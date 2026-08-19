use std::{env, path::PathBuf};

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if let Some(unknown) = args
        .iter()
        .find(|arg| !matches!(arg.as_str(), "--force" | "--limits-only"))
    {
        return Err(format!("unknown option: {unknown}"));
    }
    let home = env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".grok")))
        .ok_or("HOME and GROK_HOME are unset")?;
    let record = omarchy_agents::grok::collect_record(&home);
    println!(
        "{}",
        serde_json::to_string(&record).map_err(|error| error.to_string())?
    );
    Ok(())
}
