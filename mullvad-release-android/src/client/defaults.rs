/// Default URL for the `releases`-API.
///
/// Warren ships via GitHub Releases (mirrors the desktop `mullvad-update`
/// `WARREN_RELEASES_URL`), not the upstream Mullvad API.
/// Note that this is just a proxy to _some_ of the files in [METADATA_URL].
pub const RELEASES_URL: &str = "https://api.github.com/repos/WarrenBrowse/warren-app/releases/";

/// Default URL for version metadata repository.
pub const METADATA_URL: &str =
    "https://github.com/WarrenBrowse/warren-app/releases/latest/download/";
