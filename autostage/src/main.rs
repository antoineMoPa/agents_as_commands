use common::{git_output, run_codex};
use std::path::{Path, PathBuf};

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
    ensure_no_unmerged_paths(&repo_root)?;

    let status = unstaged_status(&repo_root)?;
    if status.trim().is_empty() {
        println!("No unstaged or untracked changes found.");
        return Ok(());
    }

    let prompt = build_prompt(&repo_root, &status);
    let raw = run_codex(
        &repo_root,
        &prompt,
        "autostaging obvious changes",
        "autostage",
        CODEX_MODEL,
        CODEX_REASONING,
    )?;

    print_response(&raw);
    print_git_summary(&repo_root)?;

    Ok(())
}

fn ensure_no_unmerged_paths(repo_root: &Path) -> Result<(), String> {
    let conflicts = git_output(["diff", "--name-only", "--diff-filter=U"], repo_root)?;
    if conflicts.trim().is_empty() {
        Ok(())
    } else {
        Err(format!(
            "unmerged paths found; resolve conflicts before autostaging:\n{}",
            conflicts.trim_end()
        ))
    }
}

fn unstaged_status(repo_root: &Path) -> Result<String, String> {
    let mut sections = Vec::new();

    let unstaged = git_output(["diff", "--name-status", "--no-renames"], repo_root)?;
    if !unstaged.trim().is_empty() {
        sections.push(format!(
            "Tracked unstaged changes:\n{}",
            unstaged.trim_end()
        ));
    }

    let untracked = git_output(["ls-files", "--others", "--exclude-standard"], repo_root)?;
    if !untracked.trim().is_empty() {
        sections.push(format!("Untracked files:\n{}", untracked.trim_end()));
    }

    Ok(sections.join("\n\n"))
}

fn build_prompt(repo_root: &Path, status: &str) -> String {
    let mut prompt = String::new();
    prompt
        .push_str("You are autostaging obvious low-risk git changes in the current repository.\n");
    prompt.push_str(
        "Your job is to update only the git index. Do not edit files and do not commit.\n",
    );
    prompt.push_str("Inspect the unstaged and untracked changes yourself with git diff/status commands before staging.\n");
    prompt.push_str("Use hunk-level staging when only part of a file is obvious. You may create and apply cached patches if needed.\n\n");
    prompt.push_str("Stage only changes that are clearly logic-free and low risk:\n");
    prompt.push_str(
        "- Routine imports/includes/use statements near the top of normal files, when not funky.\n",
    );
    prompt.push_str("- Formatting-only, whitespace-only, or mechanical ordering changes.\n");
    prompt.push_str("- Test setup, fixtures, snapshots, config, or metadata tweaks that do not change product logic.\n");
    prompt.push_str(
        "- Minor documentation, comments, typo fixes, or simple constant/text updates.\n\n",
    );
    prompt.push_str("Leave for human review, even if small:\n");
    prompt.push_str(
        "- New functions, classes, methods, modules, or files containing new executable logic.\n",
    );
    prompt.push_str("- Function signature changes, new parameters, or changed return values.\n");
    prompt.push_str("- Conditions, ternaries, branching, loops, match/switch, try/catch, async/concurrency, data flow, or algorithm changes.\n");
    prompt.push_str(
        "- API/schema/database/model changes, broad refactors, renames, moves, or deletions.\n",
    );
    prompt.push_str("- Anything ambiguous. Prefer false negatives over false positives.\n\n");
    prompt.push_str("For untracked files, stage only if the whole file is obviously non-logic support material.\n");
    prompt.push_str(
        "Preserve any changes that are already staged; do not unstage or rewrite them.\n",
    );
    prompt.push_str("When finished, report exactly what you staged and what you intentionally left unstaged.\n\n");
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Initial unstaged/untracked status:\n");
    prompt.push_str(status.trim_end());
    prompt.push('\n');
    prompt
}

fn print_response(raw: &str) {
    print!("{}", raw.trim_end());
    println!();
}

fn print_git_summary(repo_root: &Path) -> Result<(), String> {
    let staged = git_output(
        ["diff", "--cached", "--name-status", "--no-renames"],
        repo_root,
    )?;
    let remaining = unstaged_status(repo_root)?;

    println!("\nCurrent staged changes:");
    if staged.trim().is_empty() {
        println!("(none)");
    } else {
        println!("{}", staged.trim_end());
    }

    println!("\nRemaining unstaged/untracked changes:");
    if remaining.trim().is_empty() {
        println!("(none)");
    } else {
        println!("{}", remaining.trim_end());
    }

    Ok(())
}
