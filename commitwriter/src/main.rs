use common::{git_output, run_opencode};
use std::path::{Path, PathBuf};

const OPENCODE_MODEL: &str = "openai/gpt-5.4-mini";
const OPENCODE_VARIANT: &str = "medium";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let repo_root = PathBuf::from(git_output(["rev-parse", "--show-toplevel"], ".")?.trim());

    let staged_name_status = git_output(
        ["diff", "--cached", "--name-status", "--no-renames"],
        &repo_root,
    )?;
    let prompt = if staged_name_status.trim().is_empty() {
        if has_unstaged_changes(&repo_root)? {
            return Err("no staged changes found".to_string());
        }

        let commit_metadata = git_output(
            ["show", "--no-color", "--no-ext-diff", "--stat", "HEAD"],
            &repo_root,
        )?;
        let commit_diff = git_output(
            ["show", "--no-color", "--no-ext-diff", "--unified=3", "HEAD"],
            &repo_root,
        )?;
        if commit_diff.trim().is_empty() {
            return Err("last commit diff is empty".to_string());
        }

        build_last_commit_prompt(&repo_root, &commit_metadata, &commit_diff)
    } else {
        let staged_diff = git_output(
            [
                "diff",
                "--cached",
                "--no-color",
                "--no-ext-diff",
                "--unified=3",
            ],
            &repo_root,
        )?;
        if staged_diff.trim().is_empty() {
            return Err("staged diff is empty".to_string());
        }

        build_staged_prompt(&repo_root, &staged_name_status, &staged_diff)
    };
    let raw = ask_opencode(&repo_root, &prompt)?;
    let suggestion = parse_suggestion(&raw)?;

    println!(
        "git commit -m \"{}\"",
        escape_double_quotes(&suggestion.commit)
    );
    println!();
    println!("{}", suggestion.pr_paragraph);

    Ok(())
}

struct Suggestion {
    commit: String,
    pr_paragraph: String,
}

fn has_unstaged_changes(repo_root: &Path) -> Result<bool, String> {
    let unstaged_name_status = git_output(["diff", "--name-status", "--no-renames"], repo_root)?;
    if !unstaged_name_status.trim().is_empty() {
        return Ok(true);
    }

    let untracked_files = git_output(["ls-files", "--others", "--exclude-standard"], repo_root)?;
    Ok(!untracked_files.trim().is_empty())
}

fn build_staged_prompt(repo_root: &Path, name_status: &str, diff: &str) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are writing a semantic commit suggestion from staged git changes.\n");
    prompt.push_str("Return exactly two lines and nothing else:\n");
    prompt.push_str("commit: <one conventional commit subject>\n");
    prompt.push_str("pr_paragraph: <one short paragraph, 1-2 sentences, for a PR description>\n");
    prompt.push_str("Rules:\n");
    prompt.push_str("- Use only the staged diff below.\n");
    prompt.push_str("- Keep the commit concise and specific.\n");
    prompt.push_str(
        "- Keep the PR paragraph short, factual, and a little more detailed than the commit.\n\n",
    );
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Staged files:\n");
    prompt.push_str(name_status.trim_end());
    prompt.push_str("\n\nDiff:\n");
    prompt.push_str(diff.trim_end());
    prompt.push('\n');
    prompt
}

fn build_last_commit_prompt(repo_root: &Path, commit_metadata: &str, diff: &str) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are writing a PR title and description from the last git commit.\n");
    prompt.push_str("Return exactly two lines and nothing else:\n");
    prompt.push_str("commit: <one conventional commit subject based on the last commit>\n");
    prompt.push_str("pr_paragraph: <one short paragraph, 1-2 sentences, for a PR description>\n");
    prompt.push_str("Rules:\n");
    prompt.push_str("- Use only the last commit details below.\n");
    prompt.push_str("- Keep the title concise and specific.\n");
    prompt.push_str(
        "- Keep the PR paragraph short, factual, and a little more detailed than the title.\n\n",
    );
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Last commit:\n");
    prompt.push_str(commit_metadata.trim_end());
    prompt.push_str("\n\nDiff:\n");
    prompt.push_str(diff.trim_end());
    prompt.push('\n');
    prompt
}

fn ask_opencode(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_opencode(
        repo_root,
        prompt,
        "waiting for opencode",
        OPENCODE_MODEL,
        OPENCODE_VARIANT,
    )
}

fn parse_suggestion(raw: &str) -> Result<Suggestion, String> {
    let mut commit = None;
    let mut pr_paragraph = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("commit:") {
            commit = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("pr_paragraph:") {
            pr_paragraph = Some(rest.trim().to_string());
            continue;
        }
    }

    let commit = commit.ok_or_else(|| {
        format!(
            "opencode response did not include a `commit:` line\nraw output:\n{}",
            raw.trim()
        )
    })?;
    let pr_paragraph = pr_paragraph.ok_or_else(|| {
        format!(
            "opencode response did not include a `pr_paragraph:` line\nraw output:\n{}",
            raw.trim()
        )
    })?;

    Ok(Suggestion {
        commit,
        pr_paragraph,
    })
}

fn escape_double_quotes(value: &str) -> String {
    value.replace('"', "\\\"")
}
