use common::{git_output, git_output_with, run_codex};
use std::path::{Path, PathBuf};
use std::process::Command;

const CODEX_MODEL: &str = "gpt-5.4-mini";
const CODEX_REASONING: &str = "low";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let repo_root = PathBuf::from(git_output(["rev-parse", "--show-toplevel"], ".")?.trim());
    let identity = Identity::from_git_config(&repo_root)?;
    let repo_paths = collect_repo_paths(&repo_root)?;
    let date_range = DateRange::for_today()?;
    let logs = collect_matching_logs(&repo_root, &repo_paths, &identity, date_range)?;

    if logs.trim().is_empty() {
        println!(
            "No commits by {} from {} in this repo or its submodules.",
            identity.display(),
            date_range.description()
        );
        return Ok(());
    }

    let prompt = build_prompt(&repo_root, &identity, &logs, date_range);
    let raw = run_codex(
        &repo_root,
        &prompt,
        "summarizing yesterday",
        "yesterday",
        CODEX_MODEL,
        CODEX_REASONING,
    )?;

    print!("{raw}");
    if !raw.ends_with('\n') {
        println!();
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum DateRange {
    Last24Hours,
    LastFriday,
}

impl DateRange {
    fn for_today() -> Result<Self, String> {
        let output = Command::new("date")
            .arg("+%u")
            .output()
            .map_err(|e| format!("failed to check current weekday: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "failed to check current weekday with status {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        if String::from_utf8_lossy(&output.stdout).trim() == "1" {
            Ok(Self::LastFriday)
        } else {
            Ok(Self::Last24Hours)
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Last24Hours => "the last 24 hours",
            Self::LastFriday => "last Friday",
        }
    }
}

struct Identity {
    email: String,
    name: String,
}

impl Identity {
    fn from_git_config(repo_root: &Path) -> Result<Self, String> {
        let email = git_output_with(["config", "user.email"], repo_root, |_| true)?
            .trim()
            .to_string();
        let name = git_output_with(["config", "user.name"], repo_root, |_| true)?
            .trim()
            .to_string();

        if email.is_empty() && name.is_empty() {
            return Err("git user.email and user.name are both unset".to_string());
        }

        Ok(Self { email, name })
    }

    fn matches(&self, author_name: &str, author_email: &str) -> bool {
        (!self.email.is_empty() && self.email.eq_ignore_ascii_case(author_email.trim()))
            || (!self.name.is_empty() && self.name.eq_ignore_ascii_case(author_name.trim()))
    }

    fn display(&self) -> String {
        match (self.name.is_empty(), self.email.is_empty()) {
            (false, false) => format!("{} <{}>", self.name, self.email),
            (false, true) => self.name.clone(),
            (true, false) => self.email.clone(),
            (true, true) => "configured git identity".to_string(),
        }
    }
}

fn collect_repo_paths(repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = vec![repo_root.to_path_buf()];
    let submodules = git_output_with(["submodule", "status", "--recursive"], repo_root, |_| true)?;

    for line in submodules.lines() {
        if let Some(path) = parse_submodule_path(line) {
            let full_path = repo_root.join(path);
            if is_git_repo(&full_path)? {
                paths.push(full_path);
            }
        }
    }

    Ok(paths)
}

fn parse_submodule_path(line: &str) -> Option<&str> {
    line.trim_start_matches([' ', '-', '+', 'U'])
        .split_whitespace()
        .nth(1)
}

fn is_git_repo(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    let output = git_output_with(["rev-parse", "--is-inside-work-tree"], path, |_| true)?;
    Ok(output.trim() == "true")
}

fn collect_matching_logs(
    repo_root: &Path,
    repo_paths: &[PathBuf],
    identity: &Identity,
    date_range: DateRange,
) -> Result<String, String> {
    let mut sections = Vec::new();

    for repo_path in repo_paths {
        let log = match date_range {
            DateRange::Last24Hours => git_output(
                [
                    "log",
                    "--all",
                    "--since=24 hours ago",
                    "--date=iso-strict",
                    "--format=%H%x09%ad%x09%an%x09%ae%x09%B%x1e",
                ],
                repo_path,
            )?,
            DateRange::LastFriday => git_output(
                [
                    "log",
                    "--all",
                    "--since=last friday 00:00",
                    "--until=last saturday 00:00",
                    "--date=iso-strict",
                    "--format=%H%x09%ad%x09%an%x09%ae%x09%B%x1e",
                ],
                repo_path,
            )?,
        };
        let entries: Vec<String> = log
            .split('\x1e')
            .filter_map(|record| format_matching_log_record(record, identity))
            .collect();

        if entries.is_empty() {
            continue;
        }

        let label = repo_path.strip_prefix(repo_root).unwrap_or(repo_path);
        let label = if label.as_os_str().is_empty() {
            ".".to_string()
        } else {
            label.display().to_string()
        };

        sections.push(format!("Repository: {label}\n{}", entries.join("\n")));
    }

    Ok(sections.join("\n\n"))
}

fn format_matching_log_record(record: &str, identity: &Identity) -> Option<String> {
    let mut parts = record.trim().splitn(5, '\t');
    let hash = parts.next()?;
    let date = parts.next()?;
    let author_name = parts.next()?;
    let author_email = parts.next()?;
    let message = parts.next()?.trim();

    if !identity.matches(author_name, author_email) {
        return None;
    }

    let message = message
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    Some(format!(
        "- {} {} {} <{}>:\n{}",
        short_hash(hash),
        date,
        author_name,
        author_email,
        message
    ))
}

fn short_hash(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

fn build_prompt(
    repo_root: &Path,
    identity: &Identity,
    logs: &str,
    date_range: DateRange,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("Summarize in 3 executive bullet points: accomplishments from ");
    prompt.push_str(date_range.description());
    prompt.push_str(", along with any struggle bullet points if applicable. Provide a general theme for what was worked on.\n\n");
    prompt.push_str("Use only these git commit message logs from ");
    prompt.push_str(date_range.description());
    prompt.push_str(".\n\n");
    prompt.push_str("Repository root:\n");
    prompt.push_str(&format!("{}\n\n", repo_root.display()));
    prompt.push_str("Matched git identity:\n");
    prompt.push_str(&identity.display());
    prompt.push_str("\n\nCommit logs:\n");
    prompt.push_str(logs.trim_end());
    prompt.push('\n');
    prompt
}
