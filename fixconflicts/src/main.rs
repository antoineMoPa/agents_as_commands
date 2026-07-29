use common::{git_output, run_opencode};
use std::path::PathBuf;

const OPENCODE_MODEL: &str = "openai/gpt-5.6-luna";
const OPENCODE_VARIANT: &str = "medium";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let repo_root = PathBuf::from(git_output(["rev-parse", "--show-toplevel"], ".")?.trim());
    let conflicts = git_output(["diff", "--name-only", "--diff-filter=U"], &repo_root)?;

    if conflicts.trim().is_empty() {
        return Err("no merge conflicts found".to_string());
    }

    let raw = run_opencode(
        &repo_root,
        "can you fix merge conflicts?",
        "fixing merge conflicts",
        OPENCODE_MODEL,
        OPENCODE_VARIANT,
    )?;

    print!("{raw}");
    if !raw.ends_with('\n') {
        println!();
    }

    Ok(())
}
