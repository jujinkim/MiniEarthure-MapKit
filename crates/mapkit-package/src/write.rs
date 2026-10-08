//! Publish complete files without ever replacing a caller's existing artifact.
use mapkit_core::{cancellation, error, Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// No overwrite: caller must choose a new output. A temporary sibling is never a valid package.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    write_new_with(path, |file, temp| write_bytes(file, temp, bytes))
}

fn write_bytes(file: &mut File, temp: &Path, bytes: &[u8]) -> Result<()> {
    cancellation::progress("saving", 0, Some(bytes.len() as u64), "bytes");
    let mut completed = 0;
    for chunk in bytes.chunks(64 * 1024) {
        cancellation::checkpoint()?;
        file.write_all(chunk)
            .map_err(|e| save_io("write temporary file", temp, e))?;
        completed += chunk.len() as u64;
        cancellation::progress("saving", completed, Some(bytes.len() as u64), "bytes");
    }
    Ok(())
}

// Keep writing separate from publication so cancellation and partial I/O failures
// can be exercised deterministically against real temporary files.
fn write_new_with(path: &Path, write: impl FnOnce(&mut File, &Path) -> Result<()>) -> Result<()> {
    cancellation::checkpoint()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| save_io("create parent directory", parent, e))?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = parent.join(format!(
        ".mapkit-file-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| save_io("create temporary file", &temp, e))?;
    let result = (|| {
        write(&mut file, &temp)?;
        file.sync_all()
            .map_err(|e| save_io("sync temporary file", &temp, e))?;
        cancellation::checkpoint()?;
        publish_new(&temp, path)
            .map_err(|e| save_io(&format!("publish from '{}'", temp.display()), path, e))
    })();
    // Close before unlinking (also required on Windows). Android publication
    // moves the temporary file, so NotFound is the normal successful result.
    drop(file);
    match fs::remove_file(&temp) {
        Ok(()) => result,
        Err(e) if e.kind() == io::ErrorKind::NotFound => result,
        Err(e) => {
            let cleanup = save_io("remove temporary file", &temp, e);
            match result {
                Ok(()) => Err(cleanup),
                Err(mut failure) => {
                    failure.message.push_str(&format!("; {}", cleanup.message));
                    Err(failure)
                }
            }
        }
    }
}

fn save_io(stage: &str, path: &Path, cause: io::Error) -> Error {
    error("E_IO", format!("{stage} '{}': {cause}", path.display()))
}

#[cfg(not(target_os = "android"))]
fn publish_new(temp: &Path, path: &Path) -> io::Result<()> {
    fs::hard_link(temp, path)
}

#[cfg(target_os = "android")]
fn publish_new(temp: &Path, path: &Path) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(temp.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let destination = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    // Android app SELinux policy forbids hard links, including private storage.
    // Bionic's renameat2 symbol requires API 30; syscall supports our API 29
    // target without that symbol. Never fall back to an overwriting rename.
    // SAFETY: both C strings live through the call, and all five arguments have
    // the renameat2 syscall's required types. Both paths share a filesystem.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapkit_core::cancellation::CancellationToken;
    use std::{path::PathBuf, sync::Barrier};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "mapkit-write-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn entries(&self) -> Vec<PathBuf> {
            fs::read_dir(&self.0)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn complete_package_can_be_reopened() {
        let directory = Directory::new();
        let path = directory.0.join("saved.memap");
        let document: mapkit_core::MapDocument =
            serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        let bytes = crate::pack_bytes(document, Default::default()).unwrap();
        write_new(&path, &bytes).unwrap();
        let reopened = crate::read(&path).unwrap();
        assert_eq!(
            reopened.inspection.package_sha256,
            crate::read_bytes(&bytes).unwrap().inspection.package_sha256
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(directory.entries(), vec![path]);
    }

    #[test]
    fn existing_destination_is_preserved_and_publish_error_has_context() {
        let directory = Directory::new();
        let path = directory.0.join("original");
        fs::write(&path, b"original user artifact").unwrap();
        let failure = write_new(&path, b"replacement").unwrap_err();
        assert_eq!(failure.code, "E_IO");
        assert!(failure.message.contains("publish from"));
        assert!(failure.message.contains(&path.display().to_string()));
        assert!(failure.message.contains("os error"));
        assert_eq!(fs::read(&path).unwrap(), b"original user artifact");
        assert_eq!(directory.entries(), vec![path]);
    }

    #[test]
    fn concurrent_saves_publish_exactly_one_complete_file() {
        let directory = Directory::new();
        let path = directory.0.join("winner");
        let barrier = Barrier::new(8);
        let winners = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..8u8)
                .map(|value| {
                    let barrier = &barrier;
                    let path = &path;
                    scope.spawn(move || {
                        barrier.wait();
                        (value, write_new(path, &vec![value; 150_000]))
                    })
                })
                .collect();
            jobs.into_iter()
                .filter_map(|job| {
                    let (value, result) = job.join().unwrap();
                    match result {
                        Ok(()) => Some(value),
                        Err(e) => {
                            assert_eq!(e.code, "E_IO");
                            assert!(e.message.contains("publish from"));
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(winners.len(), 1);
        assert_eq!(fs::read(&path).unwrap(), vec![winners[0]; 150_000]);
        assert_eq!(directory.entries(), vec![path]);
    }

    #[test]
    fn cancelled_write_removes_partial_or_complete_temporary_file() {
        for during_write in [true, false] {
            for existing in [true, false] {
                let directory = Directory::new();
                let path = directory.0.join("result");
                if existing {
                    fs::write(&path, b"original").unwrap();
                }
                let token = CancellationToken::default();
                let failure = token
                    .run(|| {
                        write_new_with(&path, |file, temp| {
                            write_bytes(file, temp, b"first chunk")?;
                            assert_eq!(fs::read(temp).unwrap(), b"first chunk");
                            assert_eq!(
                                path.exists(),
                                existing,
                                "no partial destination is exposed"
                            );
                            token.cancel();
                            if during_write {
                                write_bytes(file, temp, b"next chunk")?;
                            }
                            Ok(())
                        })
                    })
                    .unwrap_err();
                assert_eq!(failure.code, "E_CANCELLED");
                if existing {
                    assert_eq!(fs::read(&path).unwrap(), b"original");
                    assert_eq!(directory.entries(), vec![path]);
                } else {
                    assert!(directory.entries().is_empty());
                }
            }
        }
    }

    #[test]
    fn partial_io_failure_is_cleaned_without_publishing() {
        let directory = Directory::new();
        let path = directory.0.join("result");
        let failure = write_new_with(&path, |file, temp| {
            write_bytes(file, temp, b"partial")?;
            assert!(!path.exists());
            Err(save_io(
                "write temporary file",
                temp,
                io::Error::from_raw_os_error(5),
            ))
        })
        .unwrap_err();
        assert_eq!(failure.code, "E_IO");
        assert!(failure.message.contains("write temporary file"));
        assert!(failure.message.contains(".mapkit-file-"));
        assert!(failure.message.contains("os error 5"));
        assert!(directory.entries().is_empty());
    }

    #[test]
    fn invalid_parent_and_directory_destination_are_preserved() {
        let directory = Directory::new();
        let parent = directory.0.join("file");
        fs::write(&parent, b"original").unwrap();
        let failure = write_new(&parent.join("result"), b"bytes").unwrap_err();
        assert_eq!(failure.code, "E_IO");
        assert!(failure.message.contains("create parent directory"));
        assert!(failure.message.contains(&parent.display().to_string()));
        assert!(failure.message.contains("os error"));
        assert_eq!(fs::read(&parent).unwrap(), b"original");
        assert!(write_new(&directory.0, b"bytes").is_err());
        assert_eq!(directory.entries(), vec![parent]);
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_destination_is_not_replaced_or_followed() {
        let directory = Directory::new();
        let path = directory.0.join("link");
        let missing = directory.0.join("missing");
        std::os::unix::fs::symlink(&missing, &path).unwrap();
        assert_eq!(write_new(&path, b"bytes").unwrap_err().code, "E_IO");
        assert_eq!(fs::read_link(&path).unwrap(), missing);
        assert_eq!(directory.entries(), vec![path]);
    }
}
