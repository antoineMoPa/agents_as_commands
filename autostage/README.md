# autostage

Stages only obvious, low-risk git changes and leaves the rest for human review.

It asks opencode to inspect the current repository and update only the git index.
The prompt prefers false negatives: imports, formatting, test setup, fixtures,
metadata, documentation, comments, and other logic-free tweaks can be staged;
new functions, signature changes, branching, loops, ternaries, refactors, and
ambiguous changes are left unstaged.

## Install

```bash
cargo install --path .
```

## Usage

Run from any git repository:

```bash
autostage
```

Review the staged result with `git diff --cached` before committing.
