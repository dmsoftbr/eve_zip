use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::cancel::CancelToken;
use crate::entry::ArchiveEntry;
use crate::error::EngineError;
use crate::locate::find_7zz;
use crate::parse_list::parse_slt;
use crate::runner::run_7zz;

pub struct Engine {
    bin: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZ,
    Tar,
    TarGz,
}

#[derive(Debug, Clone)]
pub struct CreateOptions {
    pub format: Format,
    pub level: u8,
    pub password: Option<String>,
    pub encrypt_names: bool,
}

fn eh_targz(archive: &Path) -> bool {
    let s = archive.to_string_lossy().to_lowercase();
    s.ends_with(".tar.gz") || s.ends_with(".tgz")
}

fn arg_senha(password: Option<&str>) -> OsString {
    // Sempre presente para o 7zz nunca abrir prompt interativo.
    let mut a = OsString::from("-p");
    if let Some(p) = password {
        a.push(p);
    }
    a
}

/// "Wrong password" sem senha fornecida significa "precisa de senha".
fn ajustar_erro_de_senha(err: EngineError, password: Option<&str>) -> EngineError {
    match (err, password) {
        (EngineError::SenhaIncorreta, None) => EngineError::SenhaNecessaria,
        (e, _) => e,
    }
}

impl Engine {
    pub fn locate() -> Result<Engine, EngineError> {
        Ok(Engine { bin: find_7zz()? })
    }

    pub fn list(
        &self,
        archive: &Path,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, EngineError> {
        if eh_targz(archive) {
            // .tar.gz é um gzip com UM tar dentro: o 7zz lista/extrai só o tar.
            // Para navegação e extração transparentes, opera-se sobre o tar interno.
            return self.com_tar_interno(archive, &CancelToken::new(), &mut |_| {}, |tar, _| {
                self.list(tar, password)
            });
        }
        let args: Vec<OsString> = vec![
            "l".into(),
            "-slt".into(),
            "-y".into(),
            arg_senha(password),
            archive.into(),
        ];
        let out = run_7zz(&self.bin, &args, &mut |_| {}, &CancelToken::new())
            .map_err(|e| ajustar_erro_de_senha(e, password))?;
        Ok(parse_slt(&out))
    }

    pub fn extract(
        &self,
        archive: &Path,
        dest: &Path,
        entries: Option<&[String]>,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        if eh_targz(archive) {
            return self.com_tar_interno(archive, cancel, on_progress, |tar, on_progress| {
                self.extract(tar, dest, entries, password, on_progress, cancel)
            });
        }
        std::fs::create_dir_all(dest)?;
        let mut o = OsString::from("-o");
        o.push(dest);
        let mut args: Vec<OsString> = vec![
            "x".into(),
            "-y".into(),
            "-bsp1".into(),
            "-bso0".into(),
            arg_senha(password),
            o,
            archive.into(),
        ];
        if let Some(sel) = entries {
            args.extend(sel.iter().map(OsString::from));
        }
        run_7zz(&self.bin, &args, on_progress, cancel)
            .map(|_| ())
            .map_err(|e| ajustar_erro_de_senha(e, password))
    }

    pub fn create(
        &self,
        archive: &Path,
        inputs: &[PathBuf],
        options: &CreateOptions,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        match options.format {
            Format::TarGz => self.create_targz(archive, inputs, on_progress, cancel),
            _ => self.create_simple(archive, inputs, options, on_progress, cancel),
        }
    }

    fn create_simple(
        &self,
        archive: &Path,
        inputs: &[PathBuf],
        options: &CreateOptions,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        let tipo = match options.format {
            Format::Zip => "-tzip",
            Format::SevenZ => "-t7z",
            Format::Tar => "-ttar",
            Format::TarGz => unreachable!("tratado em create"),
        };
        let mut args: Vec<OsString> = vec![
            "a".into(),
            tipo.into(),
            "-y".into(),
            "-bsp1".into(),
            "-bso0".into(),
        ];
        if options.format != Format::Tar {
            args.push(format!("-mx{}", options.level).into());
        }
        if let Some(pw) = &options.password {
            args.push(arg_senha(Some(pw)));
            match options.format {
                Format::Zip => args.push("-mem=AES256".into()),
                Format::SevenZ if options.encrypt_names => args.push("-mhe=on".into()),
                _ => {}
            }
        }
        args.push(archive.into());
        args.extend(inputs.iter().map(OsString::from));
        run_7zz(&self.bin, &args, on_progress, cancel).map(|_| ())
    }

    /// tar.gz em dois passos: primeiro .tar temporário, depois gzip do tar.
    fn create_targz(
        &self,
        archive: &Path,
        inputs: &[PathBuf],
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        let tmp_tar = archive.with_extension("tar.evezip-tmp");
        let passo1 = CreateOptions {
            format: Format::Tar,
            level: 0,
            password: None,
            encrypt_names: false,
        };
        // 0–50%: tar; 50–100%: gzip.
        let mut p1 = |p: u8| on_progress(p / 2);
        self.create_simple(&tmp_tar, inputs, &passo1, &mut p1, cancel)?;

        let mut args: Vec<OsString> = vec![
            "a".into(),
            "-tgzip".into(),
            "-y".into(),
            "-bsp1".into(),
            "-bso0".into(),
            archive.into(),
            (&tmp_tar).into(),
        ];
        let mut p2 = |p: u8| on_progress(50 + p / 2);
        let r = run_7zz(&self.bin, &args, &mut p2, cancel).map(|_| ());
        let _ = std::fs::remove_file(&tmp_tar);
        r
    }

    /// Extrai o .tar interno de um .tar.gz para um temp e aplica `f` sobre ele.
    /// 0–50% do progresso é a fase gzip; 50–100% fica para o `f` (via remap).
    fn com_tar_interno<T>(
        &self,
        archive: &Path,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(u8),
        f: impl FnOnce(&Path, &mut dyn FnMut(u8)) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let tmp = std::env::temp_dir().join(format!(
            "evezip-targz-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&tmp)?;

        let mut o = OsString::from("-o");
        o.push(&tmp);
        let args: Vec<OsString> = vec![
            "x".into(),
            "-y".into(),
            "-bsp1".into(),
            "-bso0".into(),
            "-p".into(),
            o,
            archive.into(),
        ];
        let mut p1 = |p: u8| on_progress(p / 2);
        let resultado = run_7zz(&self.bin, &args, &mut p1, cancel).and_then(|_| {
            let tar = std::fs::read_dir(&tmp)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.is_file())
                .ok_or_else(|| EngineError::ArchiveCorrompido("gzip sem conteúdo".into()))?;
            let mut p2 = |p: u8| on_progress(50 + p / 2);
            f(&tar, &mut p2)
        });
        let _ = std::fs::remove_dir_all(&tmp);
        resultado
    }

    pub fn test(
        &self,
        archive: &Path,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        let args: Vec<OsString> = vec![
            "t".into(),
            "-y".into(),
            "-bsp1".into(),
            "-bso0".into(),
            arg_senha(password),
            archive.into(),
        ];
        run_7zz(&self.bin, &args, on_progress, cancel)
            .map(|_| ())
            .map_err(|e| ajustar_erro_de_senha(e, password))
    }
}
