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

impl Target {
    pub fn cat(&self) -> Result<(), MyError> {
        let err = Command::new("cat").arg(&self.0).exec();

        Err(err).context(format!(
            "Failed to execute `cat` binary with argument {}",
            self.0.display()
        ))
    }


    pub fn edit(&self) -> Result<(), MyError> {
        let editor = get_editor();
        let err = Command::new(editor).arg(&self.0).exec();

        Err(err).context(format!("Failed to edit {}", self.0.display()))
    }

    pub fn help(&self) -> Result<(), MyError> {
        fn buf_read_all(mut buf: impl std::io::BufRead) -> Option<String> {
            let mut s = String::new();
            buf.read_to_string(&mut s).ok()?;
            Some(s)
        }

        if self.1.is_dir() {
            self.directory_help()
        } else {
            let help_text = get_help_file(&self.0)
                .and_then(buf_read_all)
                .or_else(|| parse_help_all(&self.0).ok())
                .unwrap_or("No help provided".to_string());

            // TODO: Use std::io::copy instead to reduce memory use.
            println!("{}", help_text.trim_end());

            Ok(())
        }
    }

    pub fn directory_help(&self) -> Result<(), MyError> {
        println!(
            "{} commands\n",
            self.0
                .file_name()
                .expect("Should be impossible given that we canonicalized the path before. Please report this as a bug.")
                .to_string_lossy()
        );

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

            let metadata = dentry.metadata()?;
            let ft = metadata.file_type();

            let mut help_text = get_help_file(dentry.path()).and_then(buf_read_line);
            if ft.is_dir() {
                print!("{BLUE_TEXT}{name:max_length$}{DEFAULT_TEXT}");
            } else {
                help_text = help_text.or_else(|| parse_help_line(dentry.path()));

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

        Ok(())
    }

    /// If successful then `exec` never returns, thus it can only return an error.
    pub fn exec<I, S>(&self, args: I) -> std::io::Error
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        Command::new(&self.0).args(args).exec()
    }
}

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
