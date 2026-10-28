# Claude Code in this workspace

Start `claude` from the repository root. The root `CLAUDE.md` holds the everyday
commands; `.claude/rules/` adds plugin/store details when matching files are opened.
The website shares `website/AGENTS.md` through its existing `CLAUDE.md` symlink.
Use `/memory` and `/context` to inspect what loaded.

This setup follows Anthropic's [best practices](https://code.claude.com/docs/en/best-practices):
short persistent instructions, concrete verification and existing CLI tools.
There are no additional hooks, MCP servers or required plugins. Existing CI handles
broad checks; running the full workspace after every edit would slow feedback.

## Spend tokens where they help

- State the symptom, scope and acceptance check in the initial prompt. For example:
  “Fix title truncation in anistream-tui at 80 columns. Add a regression test, run
  the crate tests and look at the goldens' diff. Keep the current design.”
- Ask for a plan for a substantial or ambiguous change. Small fixes can go straight
  to implementation; a mandatory planning ceremony adds context without much benefit.
- Keep one task per session. Start a fresh session for unrelated work, carrying
  forward only the decision or file references it needs. Use `/context` to diagnose
  context growth; use `/compact` when continuing a long task with noisy history.
- Request focused searches and affected-crate tests first. Preserve useful error
  output; repeatedly dumping large files or successful build logs adds little value.
- Use `/mcp` and `/plugin` to inspect enabled integrations. Keep the ones your task
  uses. Add durable repo instructions after a recurring correction, not after every
  session. Keep personal preferences in ignored `CLAUDE.local.md`.

These choices reduce unnecessary context; actual token savings depend on the task.
See [instruction loading](https://code.claude.com/docs/en/memory) for scope details.

## Optional additions

The baseline is Git, ripgrep, Cargo, Bun/npm and `gh`. Prefer these tools for local
work and GitHub tasks; this repository does not need a GitHub MCP server to get started.

The most useful optional plugin for this Rust workspace is
[rust-analyzer-lsp](https://code.claude.com/docs/en/plugins/code-intelligence), which
adds symbol navigation and diagnostics. The plugin also needs the language server.
From this repository, install it for the pinned toolchain and enable the plugin
only for your local checkout:

```sh
rustup component add rust-analyzer
rust-analyzer --version
claude plugin install rust-analyzer-lsp@claude-plugins-official --scope local
```

Restart Claude afterwards and check `/plugin` for errors. These are optional manual
setup commands, not prerequisites for the project instructions. The plugin has not
been installed by this workspace setup.

`cargo-nextest` is also optional: CI uses it to run test binaries in parallel.
`cargo test -p <crate>` remains the simplest local choice. Avoid installing more
workflow plugins until a repeated task justifies one.
