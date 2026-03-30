// TODO: Support sidecar help files

use anyhow::Context;
use std::path::PathBuf;
use std::{fs::Metadata, os::unix::process::CommandExt};

use super::{BLUE_TEXT, DEFAULT_TEXT, MyError};

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
        let err = std::process::Command::new("cat").arg(&self.0).exec();

        Err(err).context(format!(
            "Failed to execute `cat` binary with argument {}",
            self.0.display()
        ))
    }

    pub fn edit(&self) -> Result<(), MyError> {
        let editor = super::get_editor();

        let err = std::process::Command::new(editor).arg(&self.0).exec();

        Err(err).context(format!("Failed to edit {}", self.0.display()))
    }

    pub fn help(&self) -> Result<(), MyError> {
        if self.1.is_dir() {
            self.directory_help()
        } else {
            let help_text = super::parse_help_all(&self.0)?;
            let help_text = help_text.trim_end();

            println!("{}", help_text);

            Ok(())
        }
    }

    // TODO: Cleanup
    pub fn directory_help(&self) -> Result<(), MyError> {
        println!(
            "{} commands\n",
            self.0
                .file_name()
                .expect("Should be impossible given that we canonicalized the path before. Please report this as a bug.")
                .to_string_lossy()
        );

        let dentries = {
            let dentries = std::fs::read_dir(&self.0)?.filter_map(|d| d.ok());

            // Sort alphabetically by filename
            let mut dentries = dentries.collect::<Vec<_>>();
            dentries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            dentries
        };

        let max_length = dentries
            .iter()
            .map(|d| d.file_name().to_string_lossy().chars().count())
            .max()
            .unwrap_or(0);

        for dentry in dentries {
            let name = dentry.file_name();
            let name = name.to_string_lossy();

            let metadata = dentry.metadata()?;
            let ft = metadata.file_type();

            if ft.is_dir() {
                println!(
                    "{BLUE_TEXT}{0:max_length$}{DEFAULT_TEXT} -- {0} commands",
                    name,
                );
            } else if ft.is_file() {
                let help_text = super::parse_help_line(dentry.path())?;

                println!("{name:max_length$} -- {help_text}");
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
        use std::os::unix::process::CommandExt;
        std::process::Command::new(&self.0).args(args).exec()
    }
}

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
