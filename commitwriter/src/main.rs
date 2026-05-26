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

    let staged_name_status = git_output(
        ["diff", "--cached", "--name-status", "--no-renames"],
        &repo_root,
    )?;
    if staged_name_status.trim().is_empty() {
        return Err("no staged changes found".to_string());
    }

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

    let prompt = build_prompt(&repo_root, &staged_name_status, &staged_diff);
    let raw = ask_codex(&repo_root, &prompt)?;
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

fn build_prompt(repo_root: &Path, name_status: &str, diff: &str) -> String {
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

fn ask_codex(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_codex(
        repo_root,
        prompt,
        "waiting for Codex",
        "commitwriter",
        CODEX_MODEL,
        CODEX_REASONING,
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
            "codex response did not include a `commit:` line\nraw output:\n{}",
            raw.trim()
        )
    })?;
    let pr_paragraph = pr_paragraph.ok_or_else(|| {
        format!(
            "codex response did not include a `pr_paragraph:` line\nraw output:\n{}",
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
