use common::{git_output, run_codex};
use std::path::PathBuf;

const CODEX_MODEL: &str = "gpt-5.4-mini";
const CODEX_REASONING: &str = "medium";

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

    let raw = run_codex(
        &repo_root,
        "can you fix merge conflicts?",
        "fixing merge conflicts",
        "fixconflicts",
        CODEX_MODEL,
        CODEX_REASONING,
    )?;

    print!("{raw}");
    if !raw.ends_with('\n') {
        println!();
    }

    Ok(())
}
