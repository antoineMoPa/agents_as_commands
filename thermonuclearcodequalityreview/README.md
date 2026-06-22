# thermonuclearcodequalityreview

`thermonuclearcodequalityreview` reads the current git changes, calls `opencode run` with `openai/gpt-5.4-mini` at medium variant, and prints a strict code-quality review.
If the review reports findings, it will ask whether to prompt opencode again to fix them and, if confirmed, start a second opencode pass in the repo. Pass `--fix` to run that fix pass automatically.

## Install

To install the binary locally with Cargo:

```bash
cargo install --path .
```

That installs the `thermonuclearcodequalityreview` executable into your Cargo bin directory.

## Usage

After installing, run `thermonuclearcodequalityreview` from any git repository with changes you want reviewed.

To review and immediately start the fix pass when findings are present:

```bash
thermonuclearcodequalityreview --fix
```

The tool reviews the current staged, unstaged, and untracked changes together. If there are no changes, it exits early.
