# agents_as_commands

This repo contains small standalone CLI tools, each in its own Cargo package.

## Tools

- `commitwriter`: suggests a semantic commit command and a PR paragraph from staged changes.
- `thermonuclearcodequalityreview`: runs a very strict maintainability review prompt over the current git changes.

## Install

Install each tool from its package directory:

```bash
(cd commitwriter && cargo install --path .)
(cd thermonuclearcodequalityreview && cargo install --path .)
```

After installing, run `commitwriter` or `thermonuclearcodequalityreview` from any git repository with the relevant changes staged or present in the working tree.
