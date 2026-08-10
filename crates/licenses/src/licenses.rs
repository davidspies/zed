// `assets/licenses.md` is regenerated from scratch by `script/generate-licenses` on every
// bundled build, so it is embedded here rather than in `assets`: this crate is depended on
// only by the entrypoints, keeping a regenerated file from invalidating the whole build graph
// that hangs off `assets`.

use std::borrow::Cow;

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "licenses.md"]
struct LicensesAssets;

/// Attribution for the dependencies bundled into this build.
///
/// Panics unless `script/generate-licenses` ran before this crate was compiled, which the
/// bundling scripts guarantee for released builds.
pub fn open_source_licenses() -> Cow<'static, str> {
    util::asset_str::<LicensesAssets>("licenses.md")
}
