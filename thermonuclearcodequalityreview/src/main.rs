use common::{git_output, git_output_with, run_codex};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

const CODEX_MODEL: &str = "gpt-5.4-mini";
const CODEX_REASONING: &str = "medium";

struct Options {
    fix: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = parse_args()?;
    let repo_root = PathBuf::from(git_output(["rev-parse", "--show-toplevel"], ".")?.trim());
    let status = git_output(["status", "--short", "--branch"], &repo_root)?;
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
    let working_diff = git_output(
        ["diff", "--no-color", "--no-ext-diff", "--unified=3"],
        &repo_root,
    )?;
    let untracked_diff = collect_untracked_diff(&repo_root)?;

    if staged_diff.trim().is_empty()
        && working_diff.trim().is_empty()
        && untracked_diff.trim().is_empty()
    {
        return Err("no git changes found".to_string());
    }

    let prompt = build_prompt(
        &repo_root,
        &status,
        &staged_diff,
        &working_diff,
        &untracked_diff,
    );
    let raw = ask_codex(&repo_root, &prompt)?;

    print!("{raw}");
    if !raw.ends_with('\n') {
        println!();
    }

    if review_has_findings(&raw) && should_run_fix(&options)? {
        let fix_prompt = build_fix_prompt(
            &repo_root,
            &status,
            &staged_diff,
            &working_diff,
            &untracked_diff,
            &raw,
        );
        let fix_raw = ask_codex(&repo_root, &fix_prompt)?;
        print!("{fix_raw}");
        if !fix_raw.ends_with('\n') {
            println!();
        }
    }

    Ok(())
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options { fix: false };

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--fix" => options.fix = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    Ok(options)
}

fn print_usage() {
    println!("Usage: thermonuclearcodequalityreview [--fix]");
    println!();
    println!("Options:");
    println!("  --fix        Run the Codex fix pass automatically when the review finds issues");
    println!("  -h, --help   Show this help message");
}

fn build_prompt(
    repo_root: &Path,
    status: &str,
    staged_diff: &str,
    working_diff: &str,
    untracked_diff: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are performing a thermo-nuclear code quality review.\n");
    prompt.push_str("Be unusually strict about maintainability, architecture, abstraction quality, file growth, branching complexity, wrappers, optionality, and boundary cleanliness.\n");
    prompt.push_str("Do not rubber-stamp code that works but makes the codebase messier.\n");
    prompt.push_str(
        "Look for a code-judo move that deletes complexity rather than rearranging it.\n",
    );
    prompt.push_str("Prioritize structural issues over cosmetic nits.\n");
    prompt.push_str("If there are no findings, say that clearly.\n");
    prompt.push_str("When there are findings, order them from most severe to least severe and be direct about the impact.\n");
    prompt.push_str("Focus on the code in front of you, but keep an eye out for a better architecture if the current shape is obviously wrong.\n");
    prompt.push_str("Keep the review concise but substantive.\n\n");
    prompt.push_str("Review checklist:\n");
    prompt.push_str("- Can this change be made dramatically simpler by deleting branches, layers, or helpers?\n");
    prompt.push_str("- Did a file become too large or too tangled to scan comfortably?\n");
    prompt.push_str(
        "- Did the diff add ad-hoc conditionals, special cases, or flags that create spaghetti?\n",
    );
    prompt.push_str(
        "- Is the logic in the right module and layer, or is it leaking across boundaries?\n",
    );
    prompt.push_str("- Are wrappers, optionality, or abstractions hiding simple structure?\n");
    prompt.push_str(
        "- Are there avoidable sequential steps where the flow could be cleaner or more atomic?\n",
    );
    prompt.push_str("- Are existing helpers and shared utilities being reused, or is this reinventing the wheel?\n\n");
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Git status:\n");
    prompt.push_str(status.trim_end());
    prompt.push_str("\n\nStaged diff:\n");
    prompt.push_str(staged_diff.trim_end());
    prompt.push_str("\n\nWorking tree diff:\n");
    prompt.push_str(working_diff.trim_end());
    prompt.push_str("\n\nUntracked files:\n");
    prompt.push_str(untracked_diff.trim_end());
    prompt.push('\n');
    prompt
}

fn build_fix_prompt(
    repo_root: &Path,
    status: &str,
    staged_diff: &str,
    working_diff: &str,
    untracked_diff: &str,
    review: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are now the fix agent for the review below.\n");
    prompt.push_str("Apply the reported fixes directly in the repository.\n");
    prompt.push_str("Do not just restate the issues. Edit files in place.\n");
    prompt.push_str("Prefer the smallest change that makes the code meaningfully better.\n");
    prompt.push_str("If a finding is best solved by deleting code, delete it.\n");
    prompt.push_str("If a finding is best solved by moving logic, move it.\n");
    prompt.push_str("If a finding needs a test, add the smallest useful test.\n");
    prompt
        .push_str("When you are done, summarize the files changed and the main fix decisions.\n\n");
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Git status:\n");
    prompt.push_str(status.trim_end());
    prompt.push_str("\n\nStaged diff:\n");
    prompt.push_str(staged_diff.trim_end());
    prompt.push_str("\n\nWorking tree diff:\n");
    prompt.push_str(working_diff.trim_end());
    prompt.push_str("\n\nUntracked files:\n");
    prompt.push_str(untracked_diff.trim_end());
    prompt.push_str("\n\nReview to fix:\n");
    prompt.push_str(review.trim_end());
    prompt.push('\n');
    prompt
}

fn ask_codex(repo_root: &Path, prompt: &str) -> Result<String, String> {
    run_codex(
        repo_root,
        prompt,
        "reviewing with Codex",
        "thermonuclearcodequalityreview",
        CODEX_MODEL,
        CODEX_REASONING,
    )
}

fn collect_untracked_diff(repo_root: &Path) -> Result<String, String> {
    let untracked = git_output(["ls-files", "--others", "--exclude-standard"], repo_root)?;
    let mut rendered = Vec::new();

    for relative_path in untracked.lines().filter(|line| !line.trim().is_empty()) {
        let diff = git_output_with(
            [
                "diff",
                "--no-index",
                "--no-color",
                "--no-ext-diff",
                "--",
                "/dev/null",
                relative_path,
            ],
            repo_root,
            |status| status.success() || status.code() == Some(1),
        )?;
        rendered.push(diff);
    }

    Ok(rendered.join("\n"))
}

fn should_offer_fix() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn should_run_fix(options: &Options) -> Result<bool, String> {
    if options.fix {
        return Ok(true);
    }

    if !should_offer_fix() {
        return Ok(false);
    }

    prompt_yes_no("Prompt Codex to fix the reported issues? [y/N] ")
}

fn review_has_findings(review: &str) -> bool {
    let review = review.to_ascii_lowercase();
    !review.contains("no findings")
        && !review.contains("no issues")
        && !review.contains("nothing to report")
        && !review.contains("looks good")
}

fn prompt_yes_no(prompt: &str) -> Result<bool, String> {
    eprint!("{prompt}");
    io::stderr()
        .flush()
        .map_err(|e| format!("failed to flush prompt: {e}"))?;

    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(|e| format!("failed to read confirmation: {e}"))?;

    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
