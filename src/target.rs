use anyhow::Context;
use std::fs::Metadata;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

// I don't normally import everything but for such a small utility it's fine.
use super::*;

pub struct Target(PathBuf, Metadata);

impl Target {
    pub fn new(path: PathBuf) -> Result<Self, MyError> {
        let path = path.canonicalize()?;
        let metadata = std::fs::metadata(&path)?;

        Ok(Self(path, metadata))
    }

    pub fn metadata(&self) -> &Metadata {
        &self.1
    }
}

impl AsRef<std::path::Path> for Target {
    fn as_ref(&self) -> &std::path::Path {
        self.0.as_ref()
    }
}

impl Target {
    pub fn cat(&self) -> Result<(), MyError> {
        let mut f = std::fs::File::open(&self.0)?;
        std::io::copy(&mut f, &mut std::io::stdout().lock())?;

        Ok(())
    }

    pub fn create_new(&self, extra_path: impl AsRef<std::path::Path>) -> Result<(), MyError> {
        let path = self.0.join(extra_path);

        match std::fs::File::create_new(&path) {
            // File didn't exist yet and has been newly created.
            // Fill it with a template
            Ok(mut file) => {
                use std::io::Write;

                file.write_all(SCRIPT_TEMPLATE.as_bytes())?;

                use std::os::unix::fs::PermissionsExt;

                // chmod +x
                let mut perms = file.metadata()?.permissions();
                perms.set_mode(perms.mode() | 0o100);
                file.set_permissions(perms)?;
            }
            Err(e) => match e.kind() {
                // It already existed, we don't have to do anything
                std::io::ErrorKind::AlreadyExists => {}
                _ => {
                    return Err(e)
                        .context(format!("Couldn't create new file `{}`", self.0.display()));
                }
            },
        }

        Self::edit(path)
    }

    pub fn edit(path: impl AsRef<std::path::Path>) -> Result<(), MyError> {
        let path = path.as_ref();

        let editor = get_editor();
        let err = Command::new(editor).arg(&path).exec();

        Err(err).context(format!("Failed to edit {}", path.display()))
    }

    pub fn help(&self) -> Result<(), MyError> {
        let help_file = if let Some(mut h) = get_help_file(self) {
            // Use std::io::copy to reduce memory use.
            std::io::copy(&mut h, &mut std::io::stdout().lock())?;
            println!("");
            true
        } else {
            false
        };

        if !self.1.is_dir() {
            if !help_file {
                let help_text = parse_help(&self.0)
                    .map(|it| {
                        it.map(|mut l| {
                            l.push('\n');
                            l
                        })
                        .collect()
                    })
                    .unwrap_or("No help provided\n".to_string());

                println!("{}", help_text.trim_end());
            }
        } else {
            if !help_file {
                println!("{} commands\n", self.0.file_name().expect("Should be impossible given that we canonicalized the path before. Please report this as a bug.").to_string_lossy());
            }

            let mut dentries: Vec<_> = std::fs::read_dir(&self.0)?.filter_map(|d| d.ok()).collect();
            // Sort alphabetically by filename
            dentries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            let dentries = dentries;

            let max_length = dentries
                .iter()
                .map(|d| d.file_name().to_string_lossy().chars().count())
                .max()
                .unwrap_or(0);

            fn buf_read_line(buf: impl std::io::BufRead) -> Option<String> {
                buf.lines().flat_map(|l| l.ok()).next()
            }

            for dentry in dentries {
                let name = dentry.file_name();
                let name = name.to_string_lossy();

                let ft = dentry.metadata()?.file_type();

                let mut help_text = get_help_file(dentry.path()).and_then(buf_read_line);
                if ft.is_dir() {
                    print!("{BLUE_TEXT}{name:max_length$}{DEFAULT_TEXT}");
                } else {
                    help_text = help_text
                        .or_else(|| parse_help(dentry.path()).and_then(|mut it| it.next()));

                    print!("{name:max_length$}");
                }

                // Print any potential help text
                if let Some(help_text) = help_text
                    && !help_text.is_empty()
                {
                    print!(" -- {help_text}")
                }
                println!("");
            }
        }

        Ok(())
    }

    /// If successful then `exec` never returns, thus it can only return an error.
    pub fn exec<I, S>(&self, args: I) -> std::io::Error
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut cmd = Command::new(&self.0);
        cmd.args(args);

        // Log exact command we're calling to stderr
        if std::env::var_os("RUST_LOG").is_some() {
            eprintln!("{BRIGHT_YELLOW_TEXT}Running{DEFAULT_TEXT}: {:?}", cmd);
        }

        cmd.exec()
    }
}

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
