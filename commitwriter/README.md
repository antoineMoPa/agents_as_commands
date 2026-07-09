# commitwriter

`commitwriter` reads the staged and unstaged changes in the current git repository, calls `opencode run` with `openai/gpt-5.4-mini` at medium variant, and prints:

- a semantic commit subject
- a short paragraph for the PR description

If there are no staged or unstaged changes, it uses the last commit instead.

## Usage
## Install

To install the binary locally with Cargo:

```bash
cargo install --path .
```

That installs the `commitwriter` executable into your Cargo bin directory, so you can run it from any repository with staged or unstaged changes.

## Usage

After installing, run `commitwriter` from any git repository with staged or unstaged changes.

If there are no staged or unstaged changes, it writes the title and PR paragraph from the last commit.
