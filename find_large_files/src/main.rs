use common::git_output;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const DEFAULT_LIMIT: usize = 800;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let limit = parse_limit(env::args().skip(1))?;
    let current_dir = env::current_dir().map_err(|e| format!("failed to get current dir: {e}"))?;
    let files = git_output(
        ["ls-files", "--cached", "--others", "--exclude-standard"],
        &current_dir,
    )?;

    let mut matches = Vec::new();
    for file in files.lines() {
        let path = current_dir.join(file);
        if !path.is_file() {
            continue;
        }

        let line_count = count_lines(&path)?;
        if line_count > limit {
            matches.push((line_count, file.to_string()));
        }
    }

    matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    if matches.is_empty() {
        println!("No files over {limit} lines found.");
        return Ok(());
    }

    for (line_count, file) in matches {
        println!("{line_count}\t{file}");
    }

    Ok(())
}

fn parse_limit(args: impl Iterator<Item = String>) -> Result<usize, String> {
    let mut limit = DEFAULT_LIMIT;
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-n" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for -n".to_string())?;
                limit = parse_positive_limit(&value)?;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}\n\n{}", usage())),
        }
    }

    Ok(limit)
}

fn parse_positive_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| format!("invalid -n value: {value}"))?;

    if limit == 0 {
        Err("-n must be greater than 0".to_string())
    } else {
        Ok(limit)
    }
}

fn count_lines(path: &Path) -> Result<usize, String> {
    let file = File::open(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];
    let mut count = 0usize;
    let mut saw_any_bytes = false;
    let mut ended_with_newline = false;

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        if bytes_read == 0 {
            break;
        }

        saw_any_bytes = true;
        count += buffer[..bytes_read]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        ended_with_newline = buffer[bytes_read - 1] == b'\n';
    }

    if saw_any_bytes && !ended_with_newline {
        count += 1;
    }

    Ok(count)
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> &'static str {
    "Usage: find_large_files [-n LIMIT]\n\nLists repo files with more than LIMIT lines, excluding gitignored files. Defaults to 800."
}
