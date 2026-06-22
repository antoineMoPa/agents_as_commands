use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn git_output<const N: usize>(
    args: [&str; N],
    dir: impl AsRef<Path>,
) -> Result<String, String> {
    git_output_with(args, dir, |status| status.success())
}

pub fn git_output_with<const N: usize, F>(
    args: [&str; N],
    dir: impl AsRef<Path>,
    is_success: F,
) -> Result<String, String>
where
    F: FnOnce(ExitStatus) -> bool,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if !is_success(output.status) {
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

pub fn run_opencode(
    repo_root: &Path,
    prompt: &str,
    spinner_message: &str,
    model: &str,
    variant: &str,
) -> Result<String, String> {
    let prompt_file = temp_prompt_file()?;
    fs::write(&prompt_file, prompt).map_err(|e| {
        format!(
            "failed to write opencode prompt to {}: {e}",
            prompt_file.display()
        )
    })?;
    let spinner = Spinner::start(spinner_message);

    let mut command = Command::new("opencode");
    command
        .arg("run")
        .arg("--model")
        .arg(model)
        .arg("--variant")
        .arg(variant)
        .arg("--dir")
        .arg(repo_root)
        .arg("--file")
        .arg(&prompt_file)
        .arg("--dangerously-skip-permissions")
        .arg("Follow the instructions in the attached prompt file exactly.");

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run opencode: {e}"));
    let _ = fs::remove_file(&prompt_file);
    let output = output?;
    spinner.stop();

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "opencode exited with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn temp_prompt_file() -> Result<PathBuf, String> {
    let mut path = env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock is before the unix epoch: {e}"))?
        .as_nanos();
    path.push(format!(
        "opencode-prompt-{stamp}-{}.txt",
        std::process::id()
    ));
    Ok(path)
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
            eprintln!();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
            eprintln!();
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
