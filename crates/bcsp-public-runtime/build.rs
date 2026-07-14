use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DIST_ENVIRONMENT_VARIABLE: &str = "BCSP_PUBLIC_WEB_DIST";
const REQUIRED_FILES: [&str; 4] = [
    "public.html",
    "asset-manifest.json",
    "capability-manifest.json",
    "module-manifest.json",
];

fn main() {
    println!("cargo:rerun-if-env-changed={DIST_ENVIRONMENT_VARIABLE}");

    let manifest_dir = required_environment_path("CARGO_MANIFEST_DIR");
    let output_dir = required_environment_path("OUT_DIR").join("web-assets");
    let profile = env::var("PROFILE").unwrap_or_default();
    let configured_dist = env::var_os(DIST_ENVIRONMENT_VARIABLE);
    let default_dist = manifest_dir.join("../../frontend/dist/public");
    let fallback_dist = manifest_dir.join("assets/debug-public-web-dist");

    let source = match configured_dist {
        Some(path) => resolve_configured_path(&manifest_dir, PathBuf::from(path)),
        None if is_directory(&default_dist) => default_dist,
        None if profile != "release" => fallback_dist,
        None => panic!(
            "public Vite distribution is required for release builds at {} (or set {DIST_ENVIRONMENT_VARIABLE})",
            default_dist.display()
        ),
    };

    println!("cargo:rerun-if-changed={}", source.display());
    let runtime_assets =
        validate_distribution(&source, profile == "release").unwrap_or_else(|error| {
            panic!(
                "invalid public Vite distribution at {}: {error}",
                source.display()
            )
        });

    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).unwrap_or_else(|error| {
            panic!(
                "failed to clear embedded public web output {}: {error}",
                output_dir.display()
            )
        });
    }
    copy_runtime_distribution(&source, &output_dir, &runtime_assets).unwrap_or_else(|error| {
        panic!(
            "failed to copy public Vite distribution from {} to {}: {error}",
            source.display(),
            output_dir.display()
        )
    });
}

fn required_environment_path(name: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("Cargo did not provide {name}"))
}

fn resolve_configured_path(manifest_dir: &Path, configured: PathBuf) -> PathBuf {
    if configured.is_absolute() {
        configured
    } else {
        manifest_dir.join(configured)
    }
}

fn is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| !dangerous_metadata(&metadata) && metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn validate_distribution(
    source: &Path,
    require_release_distribution: bool,
) -> io::Result<BTreeSet<String>> {
    require_safe_directory(source)?;
    let mut source_map_count = 0_usize;
    validate_tree(source, &mut |path| {
        if path.extension() == Some(OsStr::new("map")) {
            source_map_count += 1;
        }
    })?;
    if source_map_count != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source maps are forbidden in the embedded public distribution",
        ));
    }
    for required in REQUIRED_FILES {
        require_nonempty_regular_file(&source.join(required))?;
    }

    let asset_manifest = fs::read_to_string(source.join("asset-manifest.json"))?;
    let capability_manifest = fs::read_to_string(source.join("capability-manifest.json"))?;
    let module_manifest = fs::read_to_string(source.join("module-manifest.json"))?;
    for (name, manifest) in [
        ("asset-manifest.json", asset_manifest.as_str()),
        ("capability-manifest.json", capability_manifest.as_str()),
        ("module-manifest.json", module_manifest.as_str()),
    ] {
        let trimmed = manifest.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} is not a JSON object"),
            ));
        }
    }
    if !manifest_targets_public(&capability_manifest)
        || !manifest_targets_public(&module_manifest)
        || module_manifest.contains("src/ui/local/")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public manifests do not describe a public-only target",
        ));
    }
    if compact_json(&asset_manifest)
        .matches("\"public.html\":")
        .count()
        != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "asset-manifest.json must contain exactly one public.html entry",
        ));
    }
    if require_release_distribution
        && !compact_json(&capability_manifest).contains("\"readiness\":\"UI_INTEGRATION_COMPLETE\"")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "release builds require the verified public UI integration distribution",
        ));
    }
    let runtime_assets = extract_manifest_asset_paths(&asset_manifest)?;

    let html = fs::read_to_string(source.join("public.html"))?;
    if !html.contains("<script") || !html.contains("type=\"module\"") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public.html does not contain a Vite module script",
        ));
    }
    if html.contains("id=\"bcsp-bootstrap\"") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public.html must not contain a pre-rendered bcsp-bootstrap payload",
        ));
    }
    if html.matches("<html").count() != 1 || html.matches("</body>").count() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public.html must contain exactly one html element and one body closing tag",
        ));
    }
    let module_asset = module_asset_reference(&html).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "public.html module script must reference a relative assets/*.js bundle",
        )
    })?;
    if !runtime_assets.contains(module_asset) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public.html module script is not reachable from asset-manifest.json",
        ));
    }

    let assets = source.join("assets");
    require_safe_directory(&assets)?;
    if !runtime_assets.iter().any(|path| path.ends_with(".js")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "assets must contain at least one JavaScript bundle",
        ));
    }
    for asset in &runtime_assets {
        let path = source.join(asset);
        require_nonempty_regular_file(&path)?;
        if asset.ends_with(".js")
            && fs::read(&path)?
                .windows(b"sourceMappingURL=".len())
                .any(|window| window == b"sourceMappingURL=")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JavaScript bundle references a source map: {asset}"),
            ));
        }
    }
    Ok(runtime_assets)
}

fn manifest_targets_public(manifest: &str) -> bool {
    compact_json(manifest).contains("\"target\":\"public\"")
}

fn compact_json(manifest: &str) -> String {
    manifest
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>()
}

fn extract_manifest_asset_paths(manifest: &str) -> io::Result<BTreeSet<String>> {
    let mut assets = BTreeSet::new();
    let mut remaining = manifest;
    while let Some(offset) = remaining.find("\"assets/") {
        remaining = &remaining[offset + 1..];
        let end = remaining.find('"').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "asset-manifest.json contains an unterminated asset path",
            )
        })?;
        let path = &remaining[..end];
        if !safe_relative_path(path) || !allowed_runtime_asset(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("asset-manifest.json contains a forbidden asset path: {path}"),
            ));
        }
        assets.insert(path.to_owned());
        remaining = &remaining[end + 1..];
    }
    if assets.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "asset-manifest.json contains no runtime assets",
        ));
    }
    Ok(assets)
}

fn allowed_runtime_asset(path: &str) -> bool {
    matches!(
        path.rsplit_once('.').map(|(_, extension)| extension),
        Some(
            "avif"
                | "css"
                | "gif"
                | "ico"
                | "jpeg"
                | "jpg"
                | "js"
                | "json"
                | "mp3"
                | "ogg"
                | "png"
                | "svg"
                | "ttf"
                | "wasm"
                | "wav"
                | "webp"
                | "woff"
                | "woff2"
        )
    )
}

fn module_asset_reference(html: &str) -> Option<&str> {
    let mut rest = html;
    while let Some(script_offset) = rest.find("<script") {
        rest = &rest[script_offset..];
        let tag_end = rest.find('>')?;
        let tag = &rest[..=tag_end];
        if tag.contains("type=\"module\"") {
            let src_start = tag.find("src=\"")? + "src=\"".len();
            let src_end = src_start + tag[src_start..].find('"')?;
            let source = &tag[src_start..src_end];
            let source = source
                .strip_prefix("./")
                .or_else(|| source.strip_prefix('/'))?;
            if source.starts_with("assets/")
                && source.ends_with(".js")
                && safe_relative_path(source)
            {
                return Some(source);
            }
            return None;
        }
        rest = &rest[tag_end + 1..];
    }
    None
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', '%', '\0'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn require_safe_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    if dangerous_metadata(&metadata) || !file_type.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is not a regular directory", path.display()),
        ));
    }
    Ok(())
}

fn require_nonempty_regular_file(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    if dangerous_metadata(&metadata) || !file_type.is_file() || metadata.len() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is not a non-empty regular file", path.display()),
        ));
    }
    Ok(())
}

fn validate_tree(directory: &Path, on_file: &mut impl FnMut(&Path)) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        if dangerous_metadata(&metadata) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("symbolic links are forbidden: {}", path.display()),
            ));
        }
        if file_type.is_dir() {
            validate_tree(&path, on_file)?;
        } else if file_type.is_file() {
            on_file(&path);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported file type: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn dangerous_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || windows_reparse_point(metadata)
}

#[cfg(windows)]
fn windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
const fn windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn copy_runtime_distribution(
    source: &Path,
    destination: &Path,
    runtime_assets: &BTreeSet<String>,
) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    fs::copy(source.join("public.html"), destination.join("public.html"))?;
    for asset in runtime_assets {
        let source_path = source.join(asset);
        let destination_path = destination.join(asset);
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_path, destination_path)?;
    }
    Ok(())
}
