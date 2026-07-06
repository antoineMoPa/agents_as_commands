use common::{git_output, git_output_with, run_opencode};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const OPENCODE_MODEL: &str = "openai/gpt-5.4-mini";
const OPENCODE_VARIANT: &str = "low";

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
    let date_range = DateRange::from_args(env::args().skip(1))?;
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
    let spinner_message = format!("summarizing {}", date_range.description());
    let raw = run_opencode(
        &repo_root,
        &prompt,
        &spinner_message,
        OPENCODE_MODEL,
        OPENCODE_VARIANT,
    )?;

    print!("{raw}");
    if !raw.ends_with('\n') {
        println!();
    }

    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DateRange {
    date: Option<GitDate>,
}

#[derive(Clone, Copy, Debug)]
struct GitDate {
    year: i32,
    month: u32,
    day: u32,
}

impl DateRange {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut date = None;
        let mut back = 0;
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-b" | "--back" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing value for -b".to_string())?;
                    back = value
                        .parse::<u32>()
                        .map_err(|_| format!("invalid -b value '{value}': expected a number"))?;
                }
                "-h" | "--help" => return Err(Self::usage()),
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown option '{arg}'\n{}", Self::usage()));
                }
                _ => {
                    if date.is_some() {
                        return Err(format!("unexpected argument '{arg}'\n{}", Self::usage()));
                    }
                    date = Some(GitDate::parse(&arg)?);
                }
            }
        }

        let mut range = if let Some(date) = date {
            Self { date: Some(date) }
        } else {
            Self::for_today()?
        };

        for _ in 0..back {
            range = range.previous_weekday()?;
        }

        Ok(range)
    }

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
            Ok(Self {
                date: Some(GitDate::today()?.previous_weekday()?),
            })
        } else {
            Ok(Self { date: None })
        }
    }

    fn previous_weekday(self) -> Result<Self, String> {
        let mut date = match self.date {
            Some(date) => date,
            None => GitDate::today()?.previous_day(),
        };

        loop {
            date = date.previous_day();
            match date.weekday()? {
                1..=5 => return Ok(Self { date: Some(date) }),
                _ => {}
            }
        }
    }

    fn since(self) -> String {
        match self.date {
            Some(date) => format!("{} 00:00", date),
            None => "24 hours ago".to_string(),
        }
    }

    fn until(self) -> Option<String> {
        self.date.map(|date| format!("{} 00:00", date.next_day()))
    }

    fn description(self) -> String {
        match self.date {
            Some(date) => date.to_string(),
            None => "the last 24 hours".to_string(),
        }
    }

    fn usage() -> String {
        "usage: yesterday [YYYY-MM-DD] [-b DAYS]".to_string()
    }
}

impl GitDate {
    fn parse(value: &str) -> Result<Self, String> {
        if value.len() != 10 || &value[4..5] != "-" || &value[7..8] != "-" {
            return Err(format!("invalid date '{value}': expected YYYY-MM-DD"));
        }

        let year = value[0..4]
            .parse::<i32>()
            .map_err(|_| format!("invalid date '{value}': expected YYYY-MM-DD"))?;
        let month = value[5..7]
            .parse::<u32>()
            .map_err(|_| format!("invalid date '{value}': expected YYYY-MM-DD"))?;
        let day = value[8..10]
            .parse::<u32>()
            .map_err(|_| format!("invalid date '{value}': expected YYYY-MM-DD"))?;

        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return Err(format!("invalid date '{value}'"));
        }

        let date = Self { year, month, day };
        date.weekday()?;
        Ok(date)
    }

    fn today() -> Result<Self, String> {
        let output = Command::new("date")
            .arg("+%Y-%m-%d")
            .output()
            .map_err(|e| format!("failed to resolve today's date: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "failed to resolve today's date with status {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        Self::parse(String::from_utf8_lossy(&output.stdout).trim())
    }

    fn previous_day(self) -> Self {
        if self.day > 1 {
            return Self {
                day: self.day - 1,
                ..self
            };
        }

        let month = if self.month == 1 { 12 } else { self.month - 1 };
        let year = if self.month == 1 {
            self.year - 1
        } else {
            self.year
        };

        Self {
            year,
            month,
            day: days_in_month(year, month),
        }
    }

    fn previous_weekday(self) -> Result<Self, String> {
        let mut date = self;

        loop {
            date = date.previous_day();
            match date.weekday()? {
                1..=5 => return Ok(date),
                _ => {}
            }
        }
    }

    fn next_day(self) -> Self {
        let mut year = self.year;
        let mut month = self.month;
        let mut day = self.day + 1;

        if day > days_in_month(year, month) {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        Self { year, month, day }
    }

    fn weekday(self) -> Result<u32, String> {
        let output = Command::new("date")
            .args(["-j", "-f", "%Y-%m-%d", &self.to_string(), "+%u"])
            .output()
            .map_err(|e| format!("failed to check weekday for {self}: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("invalid date '{self}': {}", stderr.trim()));
        }

        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("failed to parse weekday for {self}"))
    }
}

impl std::fmt::Display for GitDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_date() {
        let range = DateRange::from_args(["2026-07-02".to_string()].into_iter()).unwrap();

        assert_eq!(range.since(), "2026-07-02 00:00");
        assert_eq!(range.until().unwrap(), "2026-07-03 00:00");
        assert_eq!(range.description(), "2026-07-02");
    }

    #[test]
    fn backs_up_weekdays_from_explicit_date() {
        let range = DateRange::from_args(
            ["2026-07-06".to_string(), "-b".to_string(), "1".to_string()].into_iter(),
        )
        .unwrap();

        assert_eq!(range.since(), "2026-07-03 00:00");
        assert_eq!(range.until().unwrap(), "2026-07-04 00:00");
    }

    #[test]
    fn rejects_invalid_date() {
        let err = DateRange::from_args(["2026-02-30".to_string()].into_iter()).unwrap_err();

        assert!(err.contains("invalid date"));
    }

    #[test]
    fn calculates_month_boundaries() {
        let date = GitDate::parse("2024-03-01").unwrap().previous_day();

        assert_eq!(date.to_string(), "2024-02-29");
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
        let mut args = vec![
            "log".to_string(),
            "--all".to_string(),
            format!("--since={}", date_range.since()),
            "--date=iso-strict".to_string(),
            "--format=%H%x09%ad%x09%an%x09%ae%x09%B%x1e".to_string(),
        ];

        if let Some(until) = date_range.until() {
            args.insert(3, format!("--until={until}"));
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("failed to run git: {e}"))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "git command failed with status {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                stdout.trim(),
                stderr.trim()
            ));
        }

        let log = String::from_utf8_lossy(&output.stdout).into_owned();
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
    prompt.push_str(&date_range.description());
    prompt.push_str(", along with any struggle bullet points if applicable. Provide a general theme for what was worked on.\n\n");
    prompt.push_str("Use only these git commit message logs from ");
    prompt.push_str(&date_range.description());
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
