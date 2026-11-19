[![Zed Polacode MCP server](https://mcprush.com/badge/zed-polacode-mcp/card.svg)](https://mcprush.com/to/zed-polacode-mcp?ref=badge)

# polacode-mcp-server

A thin [Zed](https://zed.dev) extension that makes the `polacode-mcp` MCP server
available in Zed's agent panel. It contains no server code: when Zed starts the
context server, the extension returns the path of a released `polacode-mcp`
binary, downloading it from the release on first use.

The server exposes one tool, `snapshot`, which renders a file or a line range as
a Polacode-style PNG code card and returns it as an image block. The renderer,
the themes and the MCP protocol live in the server, not here:

- server source and releases: <https://github.com/talhayksk/zedpolacode>
- this extension: <https://github.com/talhayksk/polacode-mcp-server>

## What it provides

| | |
| --- | --- |
| Context server | `polacode` |
| Tool | `snapshot` |
| Anything else | nothing: no slash command, no language, no theme |

## Requirements

- Zed with MCP server extension support.
- Network access the first time the server starts, to download the binary.
- For PNG output, a Chromium-based browser on the same machine: Chrome, Edge,
  Chromium or Brave. Set `POLACODE_BROWSER` (or `CHROME_PATH`) to a browser
  executable if auto-detection fails.
- Linux: the published binaries are built on Ubuntu 22.04 and need glibc 2.35 or
  newer (Debian 12, Ubuntu 22.04+ and their derivatives).
- macOS: the published binaries are not signed or notarised. Zed writes the file itself rather
  than going through a browser, which normally avoids the quarantine attribute that triggers
  Gatekeeper; a copy downloaded through a browser may be blocked.

Supported platforms and the asset each one downloads:

| Platform | Triple | Asset |
| --- | --- | --- |
| Windows x86-64 | `x86_64-pc-windows-msvc` | `polacode-mcp-x86_64-pc-windows-msvc.zip` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `polacode-mcp-aarch64-apple-darwin.zip` |
| macOS Intel | `x86_64-apple-darwin` | `polacode-mcp-x86_64-apple-darwin.zip` |
| Linux x86-64 | `x86_64-unknown-linux-gnu` | `polacode-mcp-x86_64-unknown-linux-gnu.zip` |
| Linux arm64 | `aarch64-unknown-linux-gnu` | `polacode-mcp-aarch64-unknown-linux-gnu.zip` |

Windows on arm64, Linux on x86 and macOS on x86 are not published. The extension
reports those combinations as an error instead of downloading another
architecture.

## Install as a dev extension

The extension is not in the Zed extension registry. Install it from a clone:

```sh
git clone https://github.com/talhayksk/polacode-mcp-server.git
```

1. In Zed, run `zed: install dev extension` from the command palette.
2. Select the cloned directory (the one containing `extension.toml`).
3. Open **Settings → AI → MCP Servers**. `polacode` should be listed and turn
   green once the binary has been downloaded and the server has answered.
4. Ask the agent for a snapshot, for example: *"Take a snapshot of
   `src/lib.rs`, lines 1-30."* The tool result contains the PNG.

The first start downloads about 250 KB and takes a few seconds. Later starts
reuse the cached binary.

This extension installs `polacode-mcp` 0.2.1 and only uses its MCP server mode.
The same binary also has a `card` CLI, for example `polacode-mcp card --clipboard`,
which renders a card with no model in the loop; the extension does not use it, but
you can run the downloaded binary yourself — see
<https://github.com/talhayksk/zedpolacode>.

## How the download works

The URL is built from two constants in `src/lib.rs`:

```text
https://github.com/talhayksk/zedpolacode/releases/download/mcp-v<MCP_VERSION>/polacode-mcp-<triple>.zip
```

On the first start for a given platform:

1. The extension maps `zed::current_platform()` to one of the five triples above.
2. It looks for the binary in
   `polacode-mcp-<MCP_VERSION>/<triple>/` inside the extension work directory
   that Zed assigns to this extension, and starts it if it is already there.
3. Otherwise it downloads the zip with `zed::download_file`. Zed extracts the
   archive into that directory. The archive holds a single file at its root
   (`polacode-mcp.exe` on Windows, `polacode-mcp` elsewhere), so the binary lands
   directly in the directory. If it is not there, the extension looks at most
   three directory levels down before giving up, so a change in the archive
   layout or the host's extraction behaviour does not need a code change.
4. On macOS and Linux it marks the binary executable.
5. The absolute path is returned to Zed as the context server command.

Every failure is reported with what failed and what to do: no network, a missing
release asset (404), an unsupported platform, or an archive without the binary.
The extension does not verify the release checksum; see *Known limitations*.

### Bumping the server version

`MCP_VERSION` is the only place the server version appears. To install a newer
release:

1. Change `MCP_VERSION` in `src/lib.rs` to the release version, for example
   `"0.1.1"`.
2. Bump `version` in `extension.toml` and `Cargo.toml`, and add a `CHANGELOG.md`
   entry. Zed only refreshes an installed extension when its version changes.
3. The existing cached binaries are left alone; the new version gets its own
   directory and is downloaded on next start.

The release must publish the same five asset names for the new tag
(`mcp-v<MCP_VERSION>`).

## Troubleshooting

**`polacode-mcp <version> has no build for windows/arm64`** — the platform is not
published. Use a supported platform (see the table above) or open an issue.

**`Could not download ... 404`** — the release tag or the asset for this platform
is missing. Check the URL in the error against
<https://github.com/talhayksk/zedpolacode/releases>. If only this machine is
offline, the same message appears with a network error instead; connect and
restart the server from **Settings → AI → MCP Servers**.

**The server does not turn green, or the tool is missing** — open
`zed: open log` and search for `polacode`. Errors from the extension and the
server's stderr are forwarded there. Relaunching Zed with `zed --foreground`
gives more output.

**A download was interrupted and the binary is broken** — delete the
`polacode-mcp-<MCP_VERSION>` folder inside the extension work directory (the
error message prints the relative path, and `zed: open log` shows the work
directory). The next start downloads it again.

**`snapshot` returns "no browser"** — install Chrome, Edge, Chromium or Brave, or
point `POLACODE_BROWSER` at the executable. This is the only step that needs a
browser.

Relative `path` arguments to `snapshot` resolve against the server's working
directory, which is the project root, so file paths are relative to the open
project.

## Development

```sh
cargo fmt --check
cargo test
cargo build --release --target wasm32-wasip2
```

The tests cover the pure logic: platform-to-triple mapping for the five
supported pairs and the rejection path, URL and asset construction, the
version-scoped cache directory, the binary name per operating system, and the
bounded search that locates the extracted binary. They run on the host and never
call the WASI API. CI additionally asserts that `extension.toml` keeps the
registry id `polacode-mcp-server`.

One ignored test rehearses the resolver against a directory extracted from the
real release archive:

```sh
POLACODE_MCP_UNPACK_DIR=<unpacked-release-dir> cargo test finds_the_binary_in_an_unpacked_real_release -- --ignored --nocapture
```

## Known limitations

- The downloaded archive is not checked against the release's `sha256sums.txt`.
  Transport is HTTPS and the URL is pinned to a version, but a corrupted download
  is only noticed when the binary fails to start. Deleting the cache directory
  forces a fresh download.
- The server version is pinned in the extension source. A new `polacode-mcp`
  release needs an extension release that changes `MCP_VERSION`.
- Windows on arm64 is not published by the server release.

## License

MIT, see [LICENSE](LICENSE).
