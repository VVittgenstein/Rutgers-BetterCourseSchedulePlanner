use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DIST_ENV: &str = "BCSP_LOCAL_WEB_DIST";
const REQUIRED_FILES: &[&str] = &[
    "local.html",
    "asset-manifest.json",
    "capability-manifest.json",
    "module-manifest.json",
];

fn main() {
    println!("cargo:rerun-if-env-changed={DIST_ENV}");

    if let Err(error) = prepare_web_assets() {
        panic!("could not prepare the Windows-local web assets: {error}");
    }
}

fn prepare_web_assets() -> Result<(), String> {
    let manifest_dir = required_path_env("CARGO_MANIFEST_DIR")?;
    let out_dir = required_path_env("OUT_DIR")?;
    let profile =
        env::var("PROFILE").map_err(|error| format!("PROFILE is unavailable: {error}"))?;
    let configured_dist = env::var_os(DIST_ENV);
    let default_dist = manifest_dir.join("../../frontend/dist/local");
    let fallback = manifest_dir.join("assets/shell");

    let source = match configured_dist {
        Some(value) if value.is_empty() => {
            return Err(format!("{DIST_ENV} must not be empty"));
        }
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.exists() {
                return Err(format!(
                    "the explicitly configured {DIST_ENV} directory does not exist: {}",
                    path.display()
                ));
            }
            path
        }
        None if default_dist.exists() => default_dist,
        None if profile == "release" => {
            return Err(format!(
                "the local Vite distribution is required for release builds but is missing: {}; run the local frontend build or set {DIST_ENV}",
                default_dist.display()
            ));
        }
        None => fallback.clone(),
    };

    let source_files = validate_asset_tree(&source)?;
    for path in &source_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed={}", source.display());
    let runtime_files = select_runtime_files(&source, &source_files)?;

    let destination = out_dir.join("web-assets");
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| {
            format!(
                "could not clear stale embedded assets at {}: {error}",
                destination.display()
            )
        })?;
    }
    fs::create_dir_all(&destination).map_err(|error| {
        format!(
            "could not create embedded asset directory {}: {error}",
            destination.display()
        )
    })?;

    copy_asset_tree(&source, &destination, &runtime_files)?;
    normalize_root_asset_urls(&destination.join("local.html"))?;

    Ok(())
}

fn required_path_env(name: &str) -> Result<PathBuf, String> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} is unavailable or empty"))
}

fn validate_asset_tree(root: &Path) -> Result<Vec<PathBuf>, String> {
    validate_directory(root)?;

    let mut files = Vec::new();
    collect_regular_files(root, root, &mut files)?;
    files.sort();

    for required in REQUIRED_FILES {
        let path = root.join(required);
        if !files.iter().any(|candidate| candidate == &path) {
            return Err(format!(
                "the local web distribution is missing required file {}",
                path.display()
            ));
        }
    }

    let assets = root.join("assets");
    validate_directory(&assets)?;
    if let Some(source_map) = files
        .iter()
        .find(|path| path.extension() == Some(OsStr::new("map")))
    {
        return Err(format!(
            "source maps must not be embedded in the Windows-local runtime: {}",
            source_map.display()
        ));
    }
    let has_javascript_entry = files.iter().any(|path| {
        path.parent() == Some(assets.as_path()) && path.extension() == Some(OsStr::new("js"))
    });
    if !has_javascript_entry {
        return Err(format!(
            "the local web distribution must contain at least one assets/*.js file under {}",
            assets.display()
        ));
    }

    Ok(files)
}

fn select_runtime_files(root: &Path, source_files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    validate_manifest(
        &root.join("asset-manifest.json"),
        &["\"local.html\"", "\"file\""],
    )?;
    validate_manifest(
        &root.join("capability-manifest.json"),
        &[
            "\"schemaVersion\"",
            "\"target\"",
            "\"local\"",
            "\"allowedCapabilities\"",
            "\"allowedRoutes\"",
        ],
    )?;
    validate_manifest(
        &root.join("module-manifest.json"),
        &[
            "\"schemaVersion\"",
            "\"target\"",
            "\"local\"",
            "\"modules\"",
        ],
    )?;

    let asset_manifest_path = root.join("asset-manifest.json");
    let asset_manifest = fs::read_to_string(&asset_manifest_path).map_err(|error| {
        format!(
            "could not read {} as UTF-8: {error}",
            asset_manifest_path.display()
        )
    })?;
    let referenced_assets = manifest_referenced_assets(&asset_manifest, &asset_manifest_path)?;
    if !referenced_assets
        .iter()
        .any(|path| path.extension() == Some(OsStr::new("js")))
    {
        return Err(format!(
            "{} does not reference a JavaScript runtime asset",
            asset_manifest_path.display()
        ));
    }

    let local_html_path = root.join("local.html");
    let local_html = fs::read_to_string(&local_html_path).map_err(|error| {
        format!(
            "could not read {} as UTF-8: {error}",
            local_html_path.display()
        )
    })?;
    let module_entry = html_module_entry(&local_html, &local_html_path)?;
    if !referenced_assets.iter().any(|path| path == &module_entry) {
        return Err(format!(
            "the Vite module entry {} is not reachable from {}",
            module_entry.display(),
            asset_manifest_path.display()
        ));
    }

    let mut selected = vec![local_html_path];
    for relative in referenced_assets {
        let source = root.join(&relative);
        if !source_files.iter().any(|candidate| candidate == &source) {
            return Err(format!(
                "{} references missing or non-regular runtime asset {}",
                asset_manifest_path.display(),
                source.display()
            ));
        }
        selected.push(source);
    }
    selected.sort();
    selected.dedup();
    Ok(selected)
}

fn validate_manifest(path: &Path, required_markers: &[&str]) -> Result<(), String> {
    validate_regular_file(path)?;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read {} as UTF-8: {error}", path.display()))?;
    let trimmed = contents.trim();
    if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
        return Err(format!("manifest is not a JSON object: {}", path.display()));
    }
    for marker in required_markers {
        if !contents.contains(marker) {
            return Err(format!(
                "manifest {} is missing required marker {marker}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn manifest_referenced_assets(manifest: &str, path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut assets = Vec::new();
    let mut remaining = manifest;
    while let Some(start) = remaining.find("\"assets/") {
        let value = &remaining[start + 1..];
        let Some(end) = value.find('"') else {
            return Err(format!(
                "manifest contains an unterminated asset path: {}",
                path.display()
            ));
        };
        let asset = &value[..end];
        if asset.contains('\\') || asset.contains('\0') {
            return Err(format!(
                "manifest contains an unsafe escaped asset path {asset:?}: {}",
                path.display()
            ));
        }
        let relative = PathBuf::from(asset);
        let mut components = relative.components();
        if components
            .next()
            .and_then(|component| component.as_os_str().to_str())
            != Some("assets")
            || components.any(|component| {
                !matches!(component, std::path::Component::Normal(_))
                    || matches!(component.as_os_str().to_str(), Some("." | ".."))
            })
        {
            return Err(format!(
                "manifest contains an unsafe asset path {asset:?}: {}",
                path.display()
            ));
        }
        if relative.extension() == Some(OsStr::new("map")) {
            return Err(format!(
                "manifest must not reference a source map {asset:?}: {}",
                path.display()
            ));
        }
        assets.push(relative);
        remaining = &value[end + 1..];
    }
    assets.sort();
    assets.dedup();
    if assets.is_empty() {
        return Err(format!(
            "manifest does not reference any runtime assets: {}",
            path.display()
        ));
    }
    Ok(assets)
}

fn html_module_entry(html: &str, path: &Path) -> Result<PathBuf, String> {
    let module = html
        .find("<script type=\"module\"")
        .map(|start| &html[start..])
        .ok_or_else(|| format!("Vite HTML has no module script: {}", path.display()))?;
    let source = module
        .find("src=\"")
        .map(|start| &module[start + 5..])
        .ok_or_else(|| format!("Vite module script has no src: {}", path.display()))?;
    let end = source
        .find('"')
        .ok_or_else(|| format!("Vite module src is unterminated: {}", path.display()))?;
    let source = source[..end]
        .strip_prefix("./")
        .or_else(|| source[..end].strip_prefix('/'))
        .unwrap_or(&source[..end]);
    if !source.starts_with("assets/") {
        return Err(format!(
            "Vite module src must be below assets/: {source:?} in {}",
            path.display()
        ));
    }
    Ok(PathBuf::from(source))
}

fn collect_regular_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    validate_directory(directory)?;
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read directory {}: {error}", directory.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry below {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        if !path.starts_with(root) {
            return Err(format!(
                "asset path escaped its source directory: {}",
                path.display()
            ));
        }
        let metadata = safe_metadata(&path)?;
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            collect_regular_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        } else {
            return Err(format!(
                "asset path is neither a regular file nor a directory: {}",
                path.display()
            ));
        }
    }

    Ok(())
}

fn copy_asset_tree(source: &Path, destination: &Path, files: &[PathBuf]) -> Result<(), String> {
    for source_file in files {
        let relative = source_file.strip_prefix(source).map_err(|error| {
            format!(
                "could not make {} relative to {}: {error}",
                source_file.display(),
                source.display()
            )
        })?;
        let destination_file = destination.join(relative);
        if let Some(parent) = destination_file.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("could not create directory {}: {error}", parent.display())
            })?;
        }
        fs::copy(source_file, &destination_file).map_err(|error| {
            format!(
                "could not copy {} to {}: {error}",
                source_file.display(),
                destination_file.display()
            )
        })?;
    }
    Ok(())
}

fn normalize_root_asset_urls(html_path: &Path) -> Result<(), String> {
    let html = fs::read_to_string(html_path).map_err(|error| {
        format!(
            "could not read copied Vite HTML {} as UTF-8: {error}",
            html_path.display()
        )
    })?;
    if !html
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("<!doctype")
    {
        return Err(format!(
            "copied Vite HTML does not begin with an HTML doctype: {}",
            html_path.display()
        ));
    }
    if !html.to_ascii_lowercase().contains("<head>") {
        return Err(format!(
            "copied Vite HTML does not contain <head>: {}",
            html_path.display()
        ));
    }
    if html.to_ascii_lowercase().contains("<base ") {
        return Err(format!(
            "copied Vite HTML must not use a base element because the runtime CSP forbids it: {}",
            html_path.display()
        ));
    }
    let rooted = html.replace("./assets/", "/assets/");
    fs::write(html_path, rooted).map_err(|error| {
        format!(
            "could not root copied Vite asset URLs in {}: {error}",
            html_path.display()
        )
    })
}

fn validate_directory(path: &Path) -> Result<(), String> {
    let metadata = safe_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "expected a regular directory at {}",
            path.display()
        ));
    }
    Ok(())
}

fn validate_regular_file(path: &Path) -> Result<(), String> {
    let metadata = safe_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(format!("expected a regular file at {}", path.display()));
    }
    Ok(())
}

fn safe_metadata(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| metadata_error(path, error))?;
    if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
        return Err(format!(
            "symbolic links and reparse points are not accepted in local web assets: {}",
            path.display()
        ));
    }
    Ok(metadata)
}

fn metadata_error(path: &Path, error: io::Error) -> String {
    format!("could not inspect {}: {error}", path.display())
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
