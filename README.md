# agents_as_commands

This repo contains small standalone CLI tools, each in its own Cargo package.

## Tools

- `commitwriter`: suggests a semantic commit command and a PR paragraph from staged changes, or from the last commit when the worktree has no changes.
- `autostage`: stages only obvious, low-risk git changes and leaves logic changes for human review.
- `fixconflicts`: asks Codex to fix unresolved merge conflicts in the current repository.
- `thermonuclearcodequalityreview`: runs a very strict maintainability review prompt over the current git changes.
- `yesterday`: summarizes your commits from the last 24 hours in the current repo and submodules, using last Friday when run on Monday.

## Install

Install each tool from its package directory:

```bash
(cd autostage && cargo install --path .)
(cd commitwriter && cargo install --path .)
(cd fixconflicts && cargo install --path .)
(cd thermonuclearcodequalityreview && cargo install --path .)
(cd yesterday && cargo install --path .)
```

After installing, run `autostage`, `commitwriter`, `fixconflicts`, `thermonuclearcodequalityreview`, or `yesterday` from the relevant git repository.
