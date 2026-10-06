use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Replace a file without ever truncating the previous contents.
///
/// The temporary is created in the destination directory with `create_new`,
/// flushed to stable storage, and removed automatically if replacement fails.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    write_atomic_with_mode(path, contents, None)
}

/// Atomic replacement with an optional Unix mode for the temporary and target.
pub fn write_atomic_with_mode(path: &Path, contents: &[u8], mode: Option<u32>) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    let effective_mode = mode.or_else(|| existing_mode(path));

    let mut temp = TempFile::create(parent, path, effective_mode)?;
    let temp_path = &temp.path;

    (|| {
        temp.file
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("temporary file is closed"))?
            .write_all(contents)
            .with_context(|| format!("write {}", temp_path.display()))?;
        temp.file
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("temporary file is closed"))?
            .sync_all()
            .with_context(|| format!("sync {}", temp_path.display()))?;
        drop(temp.file.take());
        fs::rename(temp_path, path).with_context(|| format!("replace {}", path.display()))?;
        #[cfg(unix)]
        {
            let directory = File::open(parent)
                .with_context(|| format!("open {} for sync", parent.display()))?;
            directory
                .sync_all()
                .with_context(|| format!("sync {}", parent.display()))?;
        }
        Ok(())
    })()
}

#[cfg(unix)]
fn existing_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .ok()
        .map(|meta| meta.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn existing_mode(_path: &Path) -> Option<u32> {
    None
}

struct TempFile {
    path: PathBuf,
    file: Option<File>,
}

impl TempFile {
    fn create(parent: &Path, target: &Path, _mode: Option<u32>) -> Result<Self> {
        let base = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");
        let pid = std::process::id();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        for _ in 0..64 {
            let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".{base}.tmp-{pid}-{now}-{n}"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            if let Some(mode) = _mode {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(mode);
            }
            match options.open(&path) {
                Ok(file) => {
                    let temp = Self {
                        path,
                        file: Some(file),
                    };
                    #[cfg(unix)]
                    if let Some(mode) = _mode {
                        use std::os::unix::fs::PermissionsExt;
                        temp.file
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("temporary file is closed"))?
                            .set_permissions(fs::Permissions::from_mode(mode))
                            .with_context(|| format!("chmod {}", temp.path.display()))?;
                    }
                    return Ok(temp);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| format!("create {}", path.display()));
                }
            }
        }
        Err(anyhow::anyhow!(
            "could not create unique temporary for {}",
            target.display()
        ))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn replacement_failure_keeps_existing_directory_and_cleans_temp() {
        let dir = tempfile_dir("failure");
        let path = dir.join("target");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("sentinel"), "old").unwrap();

        assert!(super::write_atomic(&path, b"new").is_err());
        assert_eq!(fs::read_to_string(path.join("sentinel")).unwrap(), "old");
        assert!(
            fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-"))
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_replacements_use_unique_same_directory_temporaries() {
        let dir = tempfile_dir("concurrent");
        let path = std::sync::Arc::new(dir.join("config.toml"));
        let mut threads = Vec::new();
        for i in 0..8 {
            let path = path.clone();
            threads.push(std::thread::spawn(move || {
                super::write_atomic(&path, format!("value-{i}").as_bytes()).unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let value = fs::read_to_string(&*path).unwrap();
        assert!(value.starts_with("value-"));
        assert!(
            fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-"))
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn explicit_cookie_mode_is_private_and_general_mode_is_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile_dir("permissions");
        let path = dir.join("session.toml");
        super::write_atomic_with_mode(&path, b"cookie", Some(0o600)).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        super::write_atomic(&path, b"updated").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let _ = fs::remove_dir_all(dir);
    }

    fn tempfile_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cnmplayer-atomic-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
