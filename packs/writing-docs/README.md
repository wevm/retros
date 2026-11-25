# Writing Docs

Measures the copy of each Markdown file that Git tracks: plain technical prose in short sentences and paragraphs. textlint reads each file with published rules where one exists, and local rules where none does. Code blocks, inline code, and URLs are not prose.

Its metrics go into the shared category `documentation`, which merges with the category of that name in the project and its other packs.

## Requirements

- **Bun:** Runs the script and installs textlint and its rules from the pack's lockfile.

## Add

```sh
# Pin the pack, and approve the install of its dependencies.
uphill add wevm/uphill/packs/writing-docs@main
```
