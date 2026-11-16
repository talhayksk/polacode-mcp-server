# Changelog

All notable changes to this extension are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-16

### Added

- The `polacode` context server: Zed runs the released `polacode-mcp` binary,
  which exposes one `snapshot` tool that renders a file or line range as a PNG
  code card.
- Platform mapping for the five published targets: `x86_64-pc-windows-msvc`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu` and
  `aarch64-unknown-linux-gnu`. Other platforms get an explicit error.
- First-run download from the `mcp-v0.1.0` GitHub release into a
  version-scoped directory in the extension work directory.
