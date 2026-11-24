use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(super) struct SaveFile {
    target: PathBuf,
    temporary: Option<PathBuf>,
    file: Option<File>,
}

impl SaveFile {
    pub fn open(target: &Path) -> io::Result<Self> {
        let name = target.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Dateiname fehlt"))?;
        let parent = target.parent().unwrap_or(Path::new("."));
        let random = RandomState::new();
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        for attempt in 0..128 {
            let temporary = parent.join(format!(".rpp-save-{:016x}.tmp", random.hash_one(attempt)));
            if temporary.file_name().is_some_and(|candidate| candidate.eq_ignore_ascii_case(name)) {
                continue;
            }
            match options.open(&temporary) {
                Ok(file) => return Ok(Self { target: target.into(), temporary: Some(temporary), file: Some(file) }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "kein freier temporärer Dateiname"))
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.as_mut().unwrap().write_all(bytes)
    }

    pub fn commit(mut self) -> io::Result<()> {
        let file = self.file.take().unwrap();
        #[cfg(unix)]
        match fs::metadata(&self.target) {
            Ok(metadata) => {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(metadata.permissions().mode() & 0o777))?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        file.sync_all()?;
        drop(file);
        fs::rename(self.temporary.as_ref().unwrap(), &self.target)?;
        self.temporary.take();
        Ok(())
    }
}

impl Drop for SaveFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Some(path) = self.temporary.take() {
            let _ = fs::remove_file(path);
        }
    }
}
