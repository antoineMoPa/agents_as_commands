use common::{git_output, run_opencode};
use std::path::{Path, PathBuf};

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
    ensure_no_unmerged_paths(&repo_root)?;

    let status = unstaged_status(&repo_root)?;
    if status.trim().is_empty() {
        println!("No unstaged or untracked changes found.");
        return Ok(());
    }

    let diff = unstaged_diff(&repo_root)?;
    let prompt = build_prompt(&repo_root, &status, &diff);
    let raw = run_opencode(
        &repo_root,
        &prompt,
        "autostaging obvious changes",
        OPENCODE_MODEL,
        OPENCODE_VARIANT,
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

fn unstaged_diff(repo_root: &Path) -> Result<String, String> {
    git_output(
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=0",
            "--no-renames",
        ],
        repo_root,
    )
}

fn build_prompt(repo_root: &Path, status: &str, diff: &str) -> String {
    let mut prompt = String::new();
    prompt
        .push_str("You are autostaging obvious low-risk git changes in the current repository.\n");
    prompt.push_str(
        "Your job is to update only the git index. Do not edit files and do not commit.\n",
    );
    prompt.push_str("Use this required workflow:\n");
    prompt.push_str("1. Inspect the provided zero-context diff, then refresh it yourself if needed with git diff --no-color --no-ext-diff --unified=0 --no-renames.\n");
    prompt.push_str("2. Treat each @@ hunk in tracked files as a separate decision. Do not make file-level decisions for tracked files.\n");
    prompt.push_str(
        "3. Build a hunk ledger internally: file, hunk header, STAGE or LEAVE, and the reason.\n",
    );
    prompt.push_str(
        "4. Stage every hunk classified STAGE. Leave every hunk classified LEAVE unstaged.\n",
    );
    prompt.push_str("5. After staging, verify with git diff --cached and git diff that only the intended hunks moved to the index.\n");
    prompt.push_str("Use git add -p, git apply --cached, or another hunk-level index operation. Do not edit files. Do not commit.\n\n");
    prompt.push_str("Your default action for safe hunks is STAGE, not defer. Only defer hunks that match the human-review list below.\n\n");
    prompt.push_str("Hard rule for imports: STAGE conventional static import/include/use/export-from changes near the top of a file. Do not classify them as behavioral because the imported symbols are used by behavioral changes elsewhere. Only leave import hunks unstaged if the import itself is unconventional, such as a dynamic/programmatic import, an import in the middle of executable code, conditional loading, side-effect-only import with unclear behavior, or generated/funky ordering.\n\n");
    prompt.push_str("Stage only changes that are clearly logic-free and low risk:\n");
    prompt.push_str(
        "- Routine imports/includes/use statements near the top of normal files, when not funky.\n",
    );
    prompt.push_str("  The imported symbol's implementation may be behavioral; the import hunk itself must still be staged if it is ordinary dependency wiring.\n");
    prompt.push_str("- Simple call-site argument wiring when the added argument is an existing local/parameter/property and the hunk does not add branching or computation.\n");
    prompt.push_str("  Do not call this behavioral just because the callee behavior changes elsewhere; leave the callee change unstaged and stage the simple call-site wiring.\n");
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
    prompt.push_str("Examples to stage from a mixed file:\n");
    prompt.push_str(
        "- Replacing one ordinary import with another ordinary import at the top of the file, even when the rest of the file contains logic changes.\n",
    );
    prompt.push_str("- Adding an already-in-scope variable to a function call, such as foo(a, b) -> foo(a, b, existingValue), while leaving the callee signature/body changes unstaged.\n\n");
    prompt.push_str("Do not output that nothing was staged if any ordinary import hunk, formatting hunk, or simple call-site wiring hunk exists. In that case, stage those hunks first. If you leave an import hunk unstaged, explicitly name the unconventional property of that import hunk.\n");
    prompt.push_str("For untracked files, stage only if the whole file is obviously non-logic support material.\n");
    prompt.push_str(
        "Preserve any changes that are already staged; do not unstage or rewrite them.\n",
    );
    prompt.push_str(
        "When finished, report concise hunk-level results. Use these headings exactly:\n",
    );
    prompt.push_str("Staged hunks:\n");
    prompt.push_str("Left unstaged:\n");
    prompt.push_str("If nothing was staged, say why no individual hunk qualified; do not give only a file-level explanation.\n\n");
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Initial unstaged/untracked status:\n");
    prompt.push_str(status.trim_end());
    prompt.push_str("\n\nTracked zero-context diff:\n");
    if diff.trim().is_empty() {
        prompt.push_str("(none)\n");
    } else {
        prompt.push_str(diff.trim_end());
        prompt.push('\n');
    }
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
