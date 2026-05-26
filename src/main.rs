use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    let output_file = temp_output_file()?;
    let spinner = Spinner::start("waiting for Codex");

    let mut child = Command::new("codex")
        .arg("exec")
        .arg("--model")
        .arg(CODEX_MODEL)
        .arg("-c")
        .arg(format!("model_reasoning_effort=\"{CODEX_REASONING}\""))
        .arg("--output-last-message")
        .arg(&output_file)
        .arg("-C")
        .arg(repo_root)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start codex: {e}"))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "failed to open codex stdin".to_string())?;
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write prompt to codex: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for codex: {e}"))?;
    spinner.stop();

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "codex exited with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }

    let raw = fs::read_to_string(&output_file).map_err(|e| {
        format!(
            "failed to read codex output from {}: {e}",
            output_file.display()
        )
    })?;
    let _ = fs::remove_file(&output_file);
    Ok(raw)
}

struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    fn start(message: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let message = message.to_string();
        let frames = spinner_frames().to_vec();

        let handle = thread::spawn(move || {
            let mut index = 0usize;

            eprint!("{message} ");
            let _ = std::io::stderr().flush();

            while !thread_stop.load(Ordering::Relaxed) {
                eprint!("\r{message} {}", frames[index % frames.len()]);
                let _ = std::io::stderr().flush();
                index += 1;
                thread::sleep(Duration::from_millis(120));
            }

            eprint!("\r{message} done      \r");
            let _ = std::io::stderr().flush();
        });

        Self {
            stop,
            handle: Some(handle),
        }
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn spinner_frames() -> &'static [&'static str] {
    if supports_unicode() {
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    } else {
        &["|", "/", "-", "\\"]
    }
}

fn supports_unicode() -> bool {
    if env::var("NO_UNICODE").is_ok() {
        return false;
    }

    match env::var("LC_ALL")
        .or_else(|_| env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase()
    {
        locale if locale.is_empty() => true,
        locale if locale.contains("utf-8") || locale.contains("utf8") => true,
        _ => false,
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
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

fn temp_output_file() -> Result<PathBuf, String> {
    let mut path = env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock is before the unix epoch: {e}"))?
        .as_nanos();
    path.push(format!("commitwriter-{stamp}-{}.txt", std::process::id()));
    Ok(path)
}

fn git_output<const N: usize>(args: [&str; N], dir: impl AsRef<Path>) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
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

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
