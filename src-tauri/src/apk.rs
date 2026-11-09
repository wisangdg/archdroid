//! APK files dropped on the window or picked in the file dialog: a single
//! `.apk`, or a split-APK bundle (`.apks`, `.xapk`, `.apkm`), which is a ZIP of
//! the base APK and its configuration splits that must be installed together.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::adb;
use crate::error::{AppError, Result};
use crate::waydroid;

/// Bundle file extensions; everything else must be a plain `.apk`.
pub(crate) const BUNDLE_EXTENSIONS: [&str; 3] = ["apks", "xapk", "apkm"];
/// What a bundle may unpack to.
struct Limits {
    /// More splits than any real app ships (base + ABI + density + languages).
    splits: usize,
    /// Uncompressed size of all splits together.
    bytes: u64,
}

const LIMITS: Limits = Limits {
    splits: 64,
    bytes: 4 * 1024 * 1024 * 1024,
};

/// Top-level folders of a `bundletool build-apks` archive that are not part
/// of the split set: `standalones/` holds full APKs for Android < 5 (a second
/// base would make `install-multiple` fail), `instant/` the instant-app splits.
const SKIPPED_BUNDLE_DIRS: [&str; 2] = ["standalones/", "instant/"];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Apk,
    Bundle,
}

pub(crate) fn kind(path: &str) -> Result<Kind> {
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if extension == "apk" {
        Ok(Kind::Apk)
    } else if BUNDLE_EXTENSIONS.contains(&extension.as_str()) {
        Ok(Kind::Bundle)
    } else {
        Err(AppError::invalid_input(format!(
            "{path} is not an .apk, .apks, .xapk or .apkm file."
        )))
    }
}

/// Install a single APK or a split-APK bundle, verified by the package manager.
pub(crate) fn install_blocking(path: String, cancel: Option<&AtomicBool>) -> Result<()> {
    match kind(&path)? {
        Kind::Apk => install_single(path, cancel),
        Kind::Bundle => install_bundle(&path, cancel),
    }
}

/// How much of each end of an APK to scan for the manifest entry. The local
/// header for `AndroidManifest.xml` is normally in the first block; the central
/// directory is at the end. This rejects text files and zips that are not APKs
/// before the slow install call, without parsing the archive.
const APK_SCAN_BYTES: u64 = 8 * 1024 * 1024;

fn contains_marker(bytes: &[u8], marker: &[u8]) -> bool {
    bytes.windows(marker.len()).any(|window| window == marker)
}

pub(crate) fn apk_looks_installable(path: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || (magic != *b"PK\x03\x04" && magic != *b"PK\x05\x06")
    {
        return Err(AppError::invalid_input(format!(
            "{path} is not an APK/ZIP archive."
        )));
    }

    let marker = b"AndroidManifest.xml";
    let mut scan = Vec::with_capacity(APK_SCAN_BYTES.min(len) as usize);
    file.seek(SeekFrom::Start(0))?;
    (&mut file).take(APK_SCAN_BYTES).read_to_end(&mut scan)?;
    if contains_marker(&scan, marker) {
        return Ok(());
    }

    if len > APK_SCAN_BYTES {
        scan.clear();
        file.seek(SeekFrom::End(-(APK_SCAN_BYTES as i64)))?;
        (&mut file).take(APK_SCAN_BYTES).read_to_end(&mut scan)?;
        if contains_marker(&scan, marker) {
            return Ok(());
        }
    }

    Err(AppError::invalid_input(format!(
        "{path} is a ZIP file without AndroidManifest.xml; it is not a valid APK."
    )))
}

/// Adb is authoritative when it is installed: `adb install` waits for the
/// Android package manager and `adb uninstall` reports the real result. Without
/// adb, `waydroid app install/remove` is used, and the obvious non-APK case is
/// rejected before the CLI can report a false success.
fn install_single(path: String, cancel: Option<&AtomicBool>) -> Result<()> {
    apk_looks_installable(&path)?;
    if adb::available() {
        adb::install_apk(&path, cancel)?;
    } else {
        waydroid::Waydroid::detect()?.app_install(&path, cancel)?;
    }
    Ok(())
}

/// Split bundles need `adb install-multiple`: `waydroid app install` takes a
/// single file. The splits are unpacked into a private directory that is
/// removed afterwards, whatever the outcome.
fn install_bundle(path: &str, cancel: Option<&AtomicBool>) -> Result<()> {
    if !adb::available() {
        return Err(AppError::not_found(
            "adb",
            "Split APK bundles (.apks, .xapk, .apkm) are installed with adb install-multiple. Install adb (android-tools) and try again.",
        ));
    }
    let work = PrivateDir::create()?;
    let splits = extract_splits(Path::new(path), work.path())?;
    for split in &splits {
        apk_looks_installable(&split.to_string_lossy())?;
    }
    let splits: Vec<String> = splits
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    adb::install_apks(&splits, cancel)?;
    Ok(())
}

/// Save an installed app's APKs (base and splits) as one `.apks` bundle in
/// Downloads, mode `0600`, and return its path. No root, so app data is not
/// included (docs/BACKUP-DESIGN.md); the bundle reinstalls with
/// [`install_blocking`].
pub(crate) fn backup_apks(package: &str) -> Result<PathBuf> {
    if !adb::available() {
        return Err(AppError::not_found(
            "adb",
            "Saving an app's APKs needs adb (android-tools).",
        ));
    }
    let work = PrivateDir::create()?;
    let apks = adb::pull_apks(package, work.path())?;
    let stamp: String = crate::bundle::format_utc(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
    .chars()
    .filter(char::is_ascii_digit)
    .collect();
    let target = crate::bundle::downloads_dir()?.join(bundle_file_name(package, &stamp));
    write_bundle(&target, &apks)?;
    Ok(target)
}

fn bundle_file_name(package: &str, stamp: &str) -> String {
    format!("{package}-{stamp}.apks")
}

/// Store `apks` (already compressed) in a new ZIP at `target`, owner-only.
///
/// The ZIP is written as `<target>.part` and linked to `target` only when
/// complete, so a failed copy (disk full) never leaves a truncated file that
/// looks like a backup. An existing `target` is never overwritten.
fn write_bundle(target: &Path, apks: &[PathBuf]) -> Result<()> {
    if target.exists() {
        return Err(AppError::Io(std::io::Error::from(
            std::io::ErrorKind::AlreadyExists,
        )));
    }
    let mut part = target.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = write_zip(&part, apks).and_then(|()| Ok(std::fs::hard_link(&part, target)?));
    let _ = std::fs::remove_file(&part);
    result
}

fn write_zip(path: &Path, apks: &[PathBuf]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    let fail = |e: zip::result::ZipError| AppError::Io(std::io::Error::other(e.to_string()));
    for apk in apks {
        let name = apk
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::invalid_input("APK file name is not UTF-8"))?;
        zip.start_file(name, options).map_err(fail)?;
        std::io::copy(&mut std::fs::File::open(apk)?, &mut zip)?;
    }
    zip.finish().map_err(fail)?;
    Ok(())
}

/// Where split bundles are unpacked: `~/.cache/archdroid`, on disk. `/tmp`
/// is RAM (tmpfs) on many desktops, and a game bundle can be gigabytes.
fn work_root() -> PathBuf {
    dirs::cache_dir()
        .map(|cache| cache.join("archdroid"))
        .unwrap_or_else(std::env::temp_dir)
}

/// Work directories older than this are leftovers of a crash.
const STALE_WORK_DIR: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// A `0700` directory under [`work_root`], removed on drop.
struct PrivateDir(PathBuf);

impl PrivateDir {
    fn create() -> Result<Self> {
        Self::create_in(&work_root())
    }

    fn create_in(root: &Path) -> Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root)?;
        remove_stale_work_dirs(root);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = root.join(format!("archdroid-splits-{}-{nanos}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Remove work directories a crashed install left behind (`/tmp` was emptied
/// at boot; the cache directory is not).
fn remove_stale_work_dirs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STALE_WORK_DIR);
        if stale
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with("archdroid-splits-")
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// A full disk while unpacking says where the space ran out.
fn unpack_error(error: std::io::Error, into: &Path) -> AppError {
    if error.kind() == std::io::ErrorKind::StorageFull {
        AppError::command_failed(format!(
            "Not enough disk space to unpack the bundle in {}. Free some space and try again.",
            into.display()
        ))
    } else {
        AppError::Io(error)
    }
}

/// Unpack every `.apk` entry of `archive` into `into` and return their paths.
///
/// Only the file name of an entry is used, so `../` or absolute entry names
/// cannot write outside `into`. Duplicate names, too many splits, oversized
/// content, and OBB game data (which `install-multiple` cannot place) are
/// refused rather than half-installed.
pub(crate) fn extract_splits(archive: &Path, into: &Path) -> Result<Vec<PathBuf>> {
    extract_splits_within(archive, into, &LIMITS)
}

fn extract_splits_within(archive: &Path, into: &Path, limits: &Limits) -> Result<Vec<PathBuf>> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| {
        AppError::invalid_input(format!(
            "{} is not a split APK bundle (ZIP): {e}",
            archive.display()
        ))
    })?;
    let mut splits: Vec<PathBuf> = Vec::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| AppError::invalid_input(format!("Unreadable bundle entry: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| AppError::invalid_input(format!("Unreadable bundle entry name: {e}")))?
            .to_string();
        if entry.is_dir() {
            continue;
        }
        if name.to_ascii_lowercase().ends_with(".obb") {
            return Err(AppError::invalid_input(format!(
                "{} contains OBB game data ({name}), which ArchDroid cannot install yet. Install the APKs and copy the OBB folder to Android/obb in the shared folder by hand.",
                archive.display()
            )));
        }
        if !name.to_ascii_lowercase().ends_with(".apk")
            || SKIPPED_BUNDLE_DIRS.iter().any(|dir| name.starts_with(dir))
        {
            continue;
        }
        let file_name = name
            .rsplit(['/', '\\'])
            .next()
            .filter(|base| !base.is_empty() && !base.starts_with('.'))
            .ok_or_else(|| AppError::invalid_input(format!("Invalid bundle entry name {name:?}")))?
            .to_string();
        if splits.len() == limits.splits {
            return Err(AppError::invalid_input(format!(
                "The bundle has more than {} APKs.",
                limits.splits
            )));
        }
        total = total.saturating_add(entry.size());
        if total > limits.bytes {
            return Err(AppError::invalid_input(format!(
                "The bundle's APKs are larger than {} MiB together.",
                limits.bytes / (1024 * 1024)
            )));
        }
        let target = into.join(&file_name);
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => {
                    AppError::invalid_input(format!("The bundle contains {file_name} twice."))
                }
                _ => unpack_error(e, into),
            })?;
        // `take` holds the copy to the declared size even if the header lies.
        let size = entry.size();
        std::io::copy(&mut (&mut entry).take(size), &mut out).map_err(|e| unpack_error(e, into))?;
        splits.push(target);
    }
    if splits.is_empty() {
        return Err(AppError::invalid_input(format!(
            "{} contains no APK.",
            archive.display()
        )));
    }
    splits.sort();
    Ok(splits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("archdroid-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn bundle(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }

    const SPLIT: &[u8] = b"PK\x03\x04AndroidManifest.xml";

    #[test]
    fn apk_check_rejects_text_and_manifestless_zips() {
        let dir = std::env::temp_dir().join(format!("archdroid-apk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let text = dir.join("text.apk");
        std::fs::write(&text, "this is not an APK\n").unwrap();
        let error = apk_looks_installable(text.to_str().unwrap()).unwrap_err();
        assert_eq!(error.code(), error::code::INVALID_INPUT);
        assert!(error.to_string().contains("not an APK/ZIP"));

        let manifestless = dir.join("manifestless.apk");
        std::fs::write(&manifestless, b"PK\x03\x04notes.txt").unwrap();
        let error = apk_looks_installable(manifestless.to_str().unwrap()).unwrap_err();
        assert_eq!(error.code(), error::code::INVALID_INPUT);
        assert!(error.to_string().contains("without AndroidManifest.xml"));

        let apk = dir.join("ok.apk");
        std::fs::write(&apk, b"PK\x03\x04AndroidManifest.xml").unwrap();
        apk_looks_installable(apk.to_str().unwrap()).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_extension_decides_single_or_bundle() {
        assert_eq!(kind("/x/Game.APK").unwrap(), Kind::Apk);
        for bundle in ["a.apks", "b.XAPK", "c.apkm"] {
            assert_eq!(kind(bundle).unwrap(), Kind::Bundle, "{bundle}");
        }
        assert_eq!(
            kind("notes.txt").unwrap_err().code(),
            error::code::INVALID_INPUT
        );
    }

    #[test]
    fn splits_are_unpacked_by_file_name_only() {
        let dir = scratch("splits-ok");
        let archive = dir.join("game.xapk");
        bundle(
            &archive,
            &[
                ("manifest.json", b"{}"),
                ("com.game.apk", SPLIT),
                ("config.arm64_v8a.apk", SPLIT),
                ("../../escape/config.xxhdpi.apk", SPLIT),
                ("icon.png", b"png"),
            ],
        );
        let out = dir.join("out");
        std::fs::create_dir(&out).unwrap();
        let splits = extract_splits(&archive, &out).unwrap();
        let names: Vec<_> = splits
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            ["com.game.apk", "config.arm64_v8a.apk", "config.xxhdpi.apk"]
        );
        assert!(splits.iter().all(|p| p.parent() == Some(out.as_path())));
        assert!(!dir.join("escape").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundles_that_cannot_install_completely_are_refused() {
        let dir = scratch("splits-bad");
        type Entries<'a> = &'a [(&'a str, &'a [u8])];
        let cases: &[(&str, Entries, &str)] = &[
            ("empty.apks", &[("readme.txt", b"x")], "contains no APK"),
            (
                "dup.apks",
                &[("a/base.apk", SPLIT), ("b/base.apk", SPLIT)],
                "twice",
            ),
            (
                "obb.xapk",
                &[
                    ("base.apk", SPLIT),
                    ("Android/obb/com.g/main.1.com.g.obb", b"x"),
                ],
                "OBB",
            ),
            (
                "hidden.apks",
                &[(".base.apk", SPLIT)],
                "Invalid bundle entry",
            ),
        ];
        for (file, entries, message) in cases {
            let archive = dir.join(file);
            bundle(&archive, entries);
            let out = dir.join(format!("out-{file}"));
            std::fs::create_dir(&out).unwrap();
            let error = extract_splits(&archive, &out).unwrap_err();
            assert_eq!(error.code(), error::code::INVALID_INPUT, "{file}");
            assert!(error.to_string().contains(message), "{file}: {error}");
        }
        let text = dir.join("text.apks");
        std::fs::write(&text, "not a zip").unwrap();
        let error = extract_splits(&text, &dir).unwrap_err();
        assert!(
            error.to_string().contains("not a split APK bundle"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_saved_bundle_is_private_and_unpacks_to_the_same_splits() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("backup");
        let pulled = [dir.join("base.apk"), dir.join("split_config.en.apk")];
        for apk in &pulled {
            std::fs::write(apk, SPLIT).unwrap();
        }
        let target = dir.join(bundle_file_name("com.game", "20261109035300"));
        assert!(target.ends_with("com.game-20261109035300.apks"));
        write_bundle(&target, &pulled).unwrap();
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        // Never overwrite an earlier backup.
        assert!(write_bundle(&target, &pulled).is_err());

        let out = dir.join("out");
        std::fs::create_dir(&out).unwrap();
        let splits = extract_splits(&target, &out).unwrap();
        assert_eq!(
            splits,
            [out.join("base.apk"), out.join("split_config.en.apk")]
        );
        assert_eq!(std::fs::read(&splits[0]).unwrap(), SPLIT);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn splits_unpack_on_disk_and_crash_leftovers_are_removed() {
        assert!(work_root().ends_with("archdroid"));
        assert!(!work_root().starts_with("/tmp") || dirs::cache_dir().is_none());

        let root = scratch("workroot");
        let stale = root.join("archdroid-splits-1-1");
        let fresh = root.join("archdroid-splits-2-2");
        let other = root.join("keep-me");
        for dir in [&stale, &fresh, &other] {
            std::fs::create_dir(dir).unwrap();
        }
        let old = std::time::SystemTime::now() - STALE_WORK_DIR * 2;
        for dir in [&stale, &other] {
            std::fs::File::open(dir).unwrap().set_modified(old).unwrap();
        }
        let work = PrivateDir::create_in(&root).unwrap();
        assert!(work.path().starts_with(&root));
        assert!(!stale.exists(), "a day-old work dir is removed");
        assert!(fresh.exists() && other.exists());
        drop(work);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bundletool_archives_install_only_the_split_set() {
        let dir = scratch("bundletool");
        let archive = dir.join("app.apks");
        bundle(
            &archive,
            &[
                ("toc.pb", b"\x0a"),
                ("splits/base-master.apk", SPLIT),
                ("splits/base-x86_64.apk", SPLIT),
                ("standalones/standalone-x86_64_hdpi.apk", SPLIT),
                ("instant/instant-base-master.apk", SPLIT),
            ],
        );
        let out = dir.join("out");
        std::fs::create_dir(&out).unwrap();
        let names: Vec<_> = extract_splits(&archive, &out)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(names, ["base-master.apk", "base-x86_64.apk"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn too_many_or_too_large_splits_are_refused() {
        let dir = scratch("limits");
        let archive = dir.join("big.apks");
        bundle(
            &archive,
            &[("a.apk", SPLIT), ("b.apk", SPLIT), ("c.apk", SPLIT)],
        );
        let cases = [
            (
                Limits {
                    splits: 2,
                    bytes: 1 << 20,
                },
                "more than 2 APKs",
            ),
            (
                Limits {
                    splits: 64,
                    bytes: 2 * SPLIT.len() as u64,
                },
                "larger than",
            ),
        ];
        for (i, (limits, message)) in cases.iter().enumerate() {
            let out = dir.join(format!("out-{i}"));
            std::fs::create_dir(&out).unwrap();
            let error = extract_splits_within(&archive, &out, limits).unwrap_err();
            assert_eq!(error.code(), error::code::INVALID_INPUT);
            assert!(error.to_string().contains(message), "{error}");
            // The refused split is never written.
            assert!(!out.join("c.apk").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_save_leaves_no_file_behind() {
        let dir = scratch("backup-fail");
        let present = dir.join("base.apk");
        std::fs::write(&present, SPLIT).unwrap();
        let target = dir.join("com.game-1.apks");
        let missing = dir.join("split_gone.apk");
        assert!(write_bundle(&target, &[present, missing]).is_err());
        assert!(!target.exists());
        assert!(
            std::fs::read_dir(&dir).unwrap().count() == 1,
            "only base.apk"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_private_directory_is_owner_only_and_removed() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("private");
        let path = {
            let dir = PrivateDir::create_in(&root).unwrap();
            let mode = std::fs::metadata(dir.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
            std::fs::write(dir.path().join("x.apk"), "x").unwrap();
            dir.path().to_path_buf()
        };
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Live: `ARCHDROID_BUNDLE=/path/app.apks cargo test live_bundle_install -- --ignored`
    /// with a running Waydroid session and adb. Installs the bundle for real.
    #[test]
    #[ignore]
    fn live_bundle_install() {
        let bundle = std::env::var("ARCHDROID_BUNDLE").expect("ARCHDROID_BUNDLE");
        install_blocking(bundle, None).expect("bundle installs");
    }

    /// Live: `ARCHDROID_BACKUP_PKG=<package> cargo test live_backup_apks -- --ignored`
    /// writes `<package>-<time>.apks` to Downloads and prints its path.
    #[test]
    #[ignore]
    fn live_backup_apks() {
        let package = std::env::var("ARCHDROID_BACKUP_PKG").expect("ARCHDROID_BACKUP_PKG");
        let path = backup_apks(&package).expect("backup");
        eprintln!("BACKUP {}", path.display());
    }
}
