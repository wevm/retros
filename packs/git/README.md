# Git

Measures commit messages and repository hygiene. commitlint reads each of the last 50 commits with the conventional rules, and `git diff --check` reads whitespace. `UPHILL_GIT_COMMITS` sets the number of commits to read.

## Requirements

- **Bun:** Runs the script and installs commitlint from the pack's lockfile.
- **Git:** A clone with enough history for the commits it reads.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/git@main
```
