# fixconflicts

`fixconflicts` asks Codex to fix merge conflicts in the current git repository.

## Install

To install the binary locally with Cargo:

```bash
cargo install --path .
```

That installs the `fixconflicts` executable into your Cargo bin directory.

## Usage

Run `fixconflicts` from inside a repository that currently has unresolved merge conflicts.

The tool fails fast if no merge conflicts are present.
