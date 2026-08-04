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

        if is_excluded_file_type(&path) {
            continue;
        }

        let Some(line_count) = count_lines(&path)? else {
            continue;
        };
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

fn is_excluded_file_type(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        // Data and source maps
        "csv" | "json" | "jsonl" | "map" | "ndjson" | "parquet" | "tsv"
        // Images
        | "avif" | "bmp" | "gif" | "heic" | "ico" | "jpeg" | "jpg" | "png" | "psd"
        | "svg" | "tif" | "tiff" | "webp"
        // Archives and compressed files
        | "7z" | "bz2" | "gz" | "rar" | "tar" | "tgz" | "xz" | "zip" | "zst"
        // Audio and video
        | "aac" | "avi" | "flac" | "m4a" | "mkv" | "mov" | "mp3" | "mp4" | "mpeg"
        | "ogg" | "wav" | "webm"
        // Documents, fonts, databases, and compiled artifacts
        | "class" | "db" | "dll" | "doc" | "docx" | "dylib" | "eot" | "exe" | "o"
        | "obj" | "odt" | "pdf" | "ppt" | "pptx" | "pyc" | "so" | "sqlite"
        | "sqlite3" | "ttf" | "wasm" | "woff" | "woff2" | "xls" | "xlsx"
    )
}

fn count_lines(path: &Path) -> Result<Option<usize>, String> {
    let file = File::open(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    count_reader_lines(BufReader::new(file))
        .map_err(|e| format!("failed to read {}: {e}", path.display()))
}

fn count_reader_lines(mut reader: impl Read) -> std::io::Result<Option<usize>> {
    let mut buffer = [0u8; 8192];
    let mut count = 0usize;
    let mut saw_any_bytes = false;
    let mut ended_with_newline = false;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        if buffer[..bytes_read].contains(&0) {
            return Ok(None);
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

    Ok(Some(count))
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> &'static str {
    "Usage: find_large_files [-n LIMIT]\n\nLists text files in the repo with more than LIMIT lines, excluding gitignored files and common data, media, archive, and binary formats. Defaults to 800."
}

#[cfg(test)]
mod tests {
    use super::{count_reader_lines, is_excluded_file_type};
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn excludes_non_source_file_types_case_insensitively() {
        for path in [
            "data.json",
            "photo.PNG",
            "bundle.zip",
            "font.woff2",
            "app.wasm",
        ] {
            assert!(is_excluded_file_type(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn keeps_source_files_and_extensionless_files() {
        for path in ["src/main.rs", "script", "config.yaml", "types.d.ts"] {
            assert!(!is_excluded_file_type(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn skips_binary_content_regardless_of_extension() {
        assert_eq!(
            count_reader_lines(Cursor::new(b"first\n\0second\n")).unwrap(),
            None
        );
    }

    #[test]
    fn counts_text_without_a_trailing_newline() {
        assert_eq!(
            count_reader_lines(Cursor::new(b"first\nsecond")).unwrap(),
            Some(2)
        );
    }
}
