//! A thin Zed extension that provides the `polacode` MCP server.
//!
//! The extension ships no server code. When Zed starts the context server, the
//! extension returns the path of a released `polacode-mcp` binary for the
//! current platform, downloading it from the GitHub release for [`MCP_VERSION`]
//! the first time it is needed.

use std::path::{Path, PathBuf};

use zed_extension_api::{self as zed, Architecture, DownloadedFileType, Os};

/// Version of the `polacode-mcp` release this extension installs.
///
/// This constant is the only place the version appears: it builds the download
/// URL and the version-scoped cache directory, so bumping it installs the next
/// release and cannot accidentally reuse the previous binary.
const MCP_VERSION: &str = "0.1.0";

/// GitHub repository (`owner/name`) that publishes the `polacode-mcp` release assets.
const SOURCE_REPO: &str = "talhayksk/zedpolacode";

/// Name of the binary inside the release archive, without the Windows suffix.
const BINARY_STEM: &str = "polacode-mcp";

/// Context server id declared in `extension.toml`.
const CONTEXT_SERVER_ID: &str = "polacode";

/// How many directory levels below the cache directory are searched for the
/// extracted binary.
///
/// The release archive holds the binary at its root, so no search is normally
/// needed. Zed extracts a zip into the directory passed to `download_file`; if
/// that ever changes and the payload ends up in a wrapper folder, this bounded
/// search still finds it. The limit keeps a wrong layout from becoming an
/// unbounded filesystem walk.
const SEARCH_DEPTH: usize = 3;

const RELEASES_PAGE: &str = "https://github.com/talhayksk/zedpolacode/releases";
const ISSUES_PAGE: &str = "https://github.com/talhayksk/polacode-mcp-server/issues";

struct PolacodeMcpServerExtension;

impl zed::Extension for PolacodeMcpServerExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        context_server_id: &zed::ContextServerId,
        _project: &zed::Project,
    ) -> zed::Result<zed::Command> {
        let requested = context_server_id.as_ref();
        if requested != CONTEXT_SERVER_ID {
            return Err(format!(
                "This extension only provides the `{CONTEXT_SERVER_ID}` context server, but Zed asked for `{requested}`. \
                 Remove the `{requested}` entry from your `context_servers` settings."
            ));
        }

        let (os, architecture) = zed::current_platform();
        let triple = target_triple(os, architecture)?;
        let binary_name = binary_name(os);
        let cache_dir = cache_dir(triple);

        let binary = match find_binary(Path::new(&cache_dir), binary_name, SEARCH_DEPTH) {
            Some(binary) => binary,
            None => download(os, triple, &cache_dir, binary_name)?,
        };

        Ok(zed::Command {
            command: absolute_path(&binary)?,
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

zed::register_extension!(PolacodeMcpServerExtension);

/// Maps the platform reported by Zed to one of the five triples the release
/// publishes. There is deliberately no fallback: running a binary for another
/// architecture fails later and less clearly.
fn target_triple(os: Os, architecture: Architecture) -> Result<&'static str, String> {
    match (os, architecture) {
        (Os::Windows, Architecture::X8664) => Ok("x86_64-pc-windows-msvc"),
        (Os::Mac, Architecture::X8664) => Ok("x86_64-apple-darwin"),
        (Os::Mac, Architecture::Aarch64) => Ok("aarch64-apple-darwin"),
        (Os::Linux, Architecture::X8664) => Ok("x86_64-unknown-linux-gnu"),
        (Os::Linux, Architecture::Aarch64) => Ok("aarch64-unknown-linux-gnu"),
        (os, architecture) => Err(format!(
            "polacode-mcp {MCP_VERSION} has no build for {}/{} ({} on {}). \
             Published builds: Windows x86-64, macOS Apple Silicon and Intel, Linux x86-64 and arm64. \
             Open an issue at {ISSUES_PAGE} if you need this platform.",
            os_label(os),
            architecture_label(architecture),
            architecture_label(architecture),
            os_label(os),
        )),
    }
}

fn os_label(os: Os) -> &'static str {
    match os {
        Os::Windows => "windows",
        Os::Mac => "macos",
        Os::Linux => "linux",
    }
}

fn architecture_label(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::X86 => "x86",
        Architecture::X8664 => "x86-64",
        Architecture::Aarch64 => "arm64",
    }
}

/// Name of the binary inside the archive, which equals the command name.
fn binary_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "polacode-mcp.exe",
        Os::Mac | Os::Linux => "polacode-mcp",
    }
}

fn asset_name(triple: &str) -> String {
    format!("{BINARY_STEM}-{triple}.zip")
}

fn download_url(triple: &str) -> String {
    format!(
        "https://github.com/{SOURCE_REPO}/releases/download/mcp-v{MCP_VERSION}/{}",
        asset_name(triple)
    )
}

/// Directory that holds the binary for one server version and one platform.
///
/// The version is part of the path so an extension update that points at a new
/// release never runs a binary downloaded for the previous one.
fn cache_dir(triple: &str) -> String {
    format!("{BINARY_STEM}-{MCP_VERSION}/{triple}")
}

fn download(os: Os, triple: &str, cache_dir: &str, binary_name: &str) -> zed::Result<PathBuf> {
    let url = download_url(triple);
    let asset = asset_name(triple);

    // `cache_dir` is relative to the extension work directory, the only place
    // an extension may write. With `DownloadedFileType::Zip` Zed treats the
    // path as the destination folder and unpacks the archive into it.
    zed::download_file(&url, cache_dir, DownloadedFileType::Zip).map_err(|error| {
        format!(
            "Could not download polacode-mcp {MCP_VERSION} for {triple} from {url}: {error}. \
             Check your network connection, check that {RELEASES_PAGE} still publishes {asset}, then try again."
        )
    })?;

    let binary = find_binary(Path::new(cache_dir), binary_name, SEARCH_DEPTH).ok_or_else(|| {
        format!(
            "Downloaded polacode-mcp {MCP_VERSION} for {triple}, but `{binary_name}` was not found in `{cache_dir}`. \
             The release archive may be malformed or its layout changed. \
             Delete the `{cache_dir}` folder in the extension work directory and try again, or report it at {ISSUES_PAGE}."
        )
    })?;

    // ZIP permission bits depend on the platform that built the archive, so do
    // not rely on them: this call is cheap and idempotent. Windows only needs
    // the `.exe` suffix.
    if os != Os::Windows {
        let path = path_string(&binary);
        zed::make_file_executable(&path).map_err(|error| {
            format!(
                "Downloaded polacode-mcp {MCP_VERSION} for {triple}, but could not make `{path}` executable: {error}. \
                 Delete the `{cache_dir}` folder in the extension work directory and try again."
            )
        })?;
    }

    Ok(binary)
}

/// Returns the absolute path of the binary for `zed::Command`.
///
/// Zed starts a stdio context server with the project root as its working
/// directory, not the extension work directory, so a relative path would be
/// resolved against the user's project. The WASI runtime's current directory is
/// the extension work directory, which turns the found relative path into an
/// absolute one.
fn absolute_path(relative: &Path) -> zed::Result<String> {
    if relative.is_absolute() {
        return Ok(path_string(relative));
    }
    let work_dir = std::env::current_dir().map_err(|error| {
        format!(
            "Could not resolve the extension work directory, so the polacode-mcp path `{}` is not usable: {error}.",
            path_string(relative)
        )
    })?;
    Ok(path_string(&work_dir.join(relative)))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Returns the binary if it exists at `dir/file_name` or at most `max_depth`
/// directory levels below `dir`.
fn find_binary(dir: &Path, file_name: &str, max_depth: usize) -> Option<PathBuf> {
    let direct = dir.join(file_name);
    if direct.is_file() {
        return Some(direct);
    }
    find_in_subdirectories(dir, file_name, max_depth)
}

fn find_in_subdirectories(dir: &Path, file_name: &str, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let mut subdirectories: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        // `DirEntry::file_type` does not follow symlinks, which keeps a
        // symlinked directory inside the work directory from causing a loop.
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    // A stable order keeps the result deterministic if an archive ever ships
    // two copies of the binary.
    subdirectories.sort();
    for subdirectory in subdirectories {
        let candidate = subdirectory.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if let Some(found) = find_in_subdirectories(&subdirectory, file_name, depth - 1) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// The five triples the release publishes, as an independent copy of the
    /// contract the tests enforce.
    const RELEASE_TRIPLES: [&str; 5] = [
        "x86_64-pc-windows-msvc",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ];

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "polacode-mcp-server-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"binary").unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn maps_all_five_supported_platforms_to_release_triples() {
        let cases = [
            (Os::Windows, Architecture::X8664, "x86_64-pc-windows-msvc"),
            (Os::Mac, Architecture::X8664, "x86_64-apple-darwin"),
            (Os::Mac, Architecture::Aarch64, "aarch64-apple-darwin"),
            (Os::Linux, Architecture::X8664, "x86_64-unknown-linux-gnu"),
            (
                Os::Linux,
                Architecture::Aarch64,
                "aarch64-unknown-linux-gnu",
            ),
        ];
        for (os, architecture, expected) in cases {
            assert_eq!(
                target_triple(os, architecture).as_deref(),
                Ok(expected),
                "{os:?}/{architecture:?}"
            );
        }
    }

    #[test]
    fn rejects_unsupported_platforms_by_name_without_substituting_a_triple() {
        let unsupported = [
            (Os::Windows, Architecture::Aarch64),
            (Os::Windows, Architecture::X86),
            (Os::Linux, Architecture::X86),
            (Os::Mac, Architecture::X86),
        ];
        for (os, architecture) in unsupported {
            let error = target_triple(os, architecture).unwrap_err();
            assert!(error.contains(os_label(os)), "{error}");
            assert!(error.contains(architecture_label(architecture)), "{error}");
            assert!(error.contains(MCP_VERSION), "{error}");
            assert!(error.contains(ISSUES_PAGE), "{error}");
            for triple in RELEASE_TRIPLES {
                assert!(
                    !error.contains(triple),
                    "the error must not offer {triple} as a fallback: {error}"
                );
            }
        }
    }

    #[test]
    fn builds_the_release_url_from_the_version_and_triple() {
        assert_eq!(
            download_url("x86_64-pc-windows-msvc"),
            "https://github.com/talhayksk/zedpolacode/releases/download/mcp-v0.1.0/polacode-mcp-x86_64-pc-windows-msvc.zip"
        );
        for triple in RELEASE_TRIPLES {
            let url = download_url(triple);
            assert_eq!(
                url,
                format!(
                    "https://github.com/talhayksk/zedpolacode/releases/download/mcp-v{MCP_VERSION}/{}",
                    asset_name(triple)
                )
            );
            assert!(url.ends_with(&format!("polacode-mcp-{triple}.zip")));
        }
    }

    #[test]
    fn keeps_each_server_version_in_its_own_cache_directory() {
        for triple in RELEASE_TRIPLES {
            let dir = cache_dir(triple);
            assert_eq!(dir, format!("polacode-mcp-{MCP_VERSION}/{triple}"));
            assert!(dir.starts_with("polacode-mcp-0.1.0/"));
            assert!(dir.ends_with(triple));
        }
    }

    #[test]
    fn names_the_binary_after_the_platform() {
        assert_eq!(binary_name(Os::Windows), "polacode-mcp.exe");
        assert_eq!(binary_name(Os::Mac), "polacode-mcp");
        assert_eq!(binary_name(Os::Linux), "polacode-mcp");
    }

    #[test]
    fn finds_the_binary_at_the_archive_root() {
        let dir = TempDir::new();
        let expected = dir.write("polacode-mcp.exe");
        assert_eq!(
            find_binary(&dir.0, "polacode-mcp.exe", SEARCH_DEPTH),
            Some(expected)
        );
    }

    #[test]
    fn finds_the_binary_in_a_nested_folder() {
        let dir = TempDir::new();
        let expected = dir.write("polacode-mcp-x86_64-pc-windows-msvc/polacode-mcp.exe");
        assert_eq!(
            find_binary(&dir.0, "polacode-mcp.exe", SEARCH_DEPTH),
            Some(expected)
        );
    }

    #[test]
    fn stops_searching_at_the_depth_limit() {
        let dir = TempDir::new();
        let expected = dir.write("a/b/c/polacode-mcp");
        // a/b/c is three levels below the cache directory.
        assert_eq!(find_binary(&dir.0, "polacode-mcp", 3), Some(expected));

        let deeper = TempDir::new();
        deeper.write("a/b/c/d/polacode-mcp");
        assert_eq!(find_binary(&deeper.0, "polacode-mcp", 3), None);
        assert_eq!(
            find_binary(&deeper.0, "polacode-mcp", 4),
            Some(deeper.0.join("a/b/c/d/polacode-mcp"))
        );
    }

    #[test]
    fn returns_nothing_when_the_binary_is_missing() {
        let dir = TempDir::new();
        dir.write("sha256sums.txt");
        assert_eq!(find_binary(&dir.0, "polacode-mcp", SEARCH_DEPTH), None);

        // A directory with the binary's name is not a runnable command.
        let namesake = TempDir::new();
        std::fs::create_dir_all(namesake.0.join("polacode-mcp")).unwrap();
        assert_eq!(find_binary(&namesake.0, "polacode-mcp", SEARCH_DEPTH), None);
    }

    /// Rehearses the resolver on an indefinite real input: a directory produced
    /// by extracting the actual release archive.
    ///
    /// ```text
    /// POLACODE_MCP_UNPACK_DIR=<dir> cargo test finds_the_binary_in_an_unpacked_real_release -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a directory extracted from the real polacode-mcp release archive"]
    fn finds_the_binary_in_an_unpacked_real_release() {
        let unpack_dir = std::env::var("POLACODE_MCP_UNPACK_DIR").expect(
            "set POLACODE_MCP_UNPACK_DIR to a directory extracted from the release archive",
        );
        let found = find_binary(Path::new(&unpack_dir), "polacode-mcp.exe", SEARCH_DEPTH)
            .or_else(|| find_binary(Path::new(&unpack_dir), "polacode-mcp", SEARCH_DEPTH))
            .expect("no polacode-mcp binary in the unpacked release");
        let size = std::fs::metadata(&found).unwrap().len();
        println!("resolved: {} ({size} bytes)", found.display());
        assert!(size > 0);
    }
}
