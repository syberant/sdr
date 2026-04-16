mod target;
use target::Target;

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context;
type MyError = anyhow::Error;

const BLUE_TEXT: &str = "\x1b[34m";
const DEFAULT_TEXT: &str = "\x1b[0m";

/// The action we are supposed to carry out.
enum Action {
    Run,
    Help,
    // TODO: Maybe merge this one into the functionality of the `--edit` flag?
    New,
    Edit,
    Cat,
    Which,
    /// Try to autocomplete from the given arguments.
    Complete,
}

fn get_root() -> Result<PathBuf, MyError> {
    if let Some(p) = std::env::var_os("SD_ROOT") {
        Ok(PathBuf::from(p))
    } else {
        let mut home = std::env::home_dir()
            .context("User's home directory could not be found, please check $HOME.")?;
        home.push("sd");
        Ok(home)
    }
}

fn get_editor() -> OsString {
    if let Some(s) = std::env::var_os("SD_EDITOR") {
        return s;
    }
    if let Some(s) = std::env::var_os("VISUAL") {
        return s;
    }

    std::env::var_os("EDITOR").unwrap_or(OsString::from("vi"))
}

fn get_help_file<P: AsRef<std::path::Path>>(path: P) -> Option<impl std::io::BufRead> {
    let path = path.as_ref();

    let help_file = if path.is_dir() {
        // Only works for directories
        path.join("help")
    } else {
        path.with_added_extension(".help")
    };

    // Read help file
    let f = std::fs::File::open(help_file).ok()?;
    Some(std::io::BufReader::new(f))
}

fn parse_help_line<P: AsRef<std::path::Path>>(path: P) -> Option<String> {
    use std::io::Read;

    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 1000];
    let read = f.read(&mut buf).ok()?;
    let buf = &buf[..read];
    let s = std::str::from_utf8(buf).unwrap_or("");

    let help_text = s
        .lines()
        // Skip the `#!` (binfmt) line
        .skip(1)
        // Skip non-comment lines and pick the first one
        .skip_while(|line| line.chars().next() != Some('#'))
        .next()
        // Remove the `#` and whitespace that follows it.
        .and_then(|line| line.get(1..))
        .unwrap_or("")
        .trim_start()
        .to_string();

    Some(help_text)
}

fn parse_help_all<P: AsRef<std::path::Path>>(path: P) -> Result<String, MyError> {
    use std::io::Read;

    let mut f = std::fs::File::open(path)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;

    let help_text = s
        .split_inclusive('\n')
        // Skip the `#!` (binfmt) line
        .skip(1)
        // Skip non-comment lines and pick the first contiguous block of comments
        .skip_while(|line| line.chars().next() != Some('#'))
        .take_while(|line| line.chars().next() == Some('#'))
        .map(|line| {
            // Remove the `#` and whitespace that follows it.
            line.get(1..).unwrap_or("").trim_start()
        });

    Ok(help_text.collect())
}

fn parse_target() -> Result<(Target, impl Iterator<Item = OsString>), MyError> {
    let mut args = std::env::args_os()
        .into_iter()
        // Skip arg0
        .skip(1)
        .peekable();

    let root_dir =
        get_root().context("Could not find root `sd` directory, consider setting $SD_ROOT")?;

    // Find the longest prefix of `args` that still leads to something that exists on the filesystem.
    let mut target = root_dir;
    while let Some(d) = args.peek() {
        target.push(d);

        if target.try_exists().unwrap_or(false) {
            args.next();
        } else {
            target.pop();
            break;
        }
    }

    Ok((Target::new(target)?, args))
}

fn main() -> Result<(), MyError> {
    let (target, args) = parse_target()?;

    let mut args = args.peekable();

    let action = match args.peek().map(|s| s.as_encoded_bytes()) {
        Some(b"--help") => Action::Help,
        Some(b"--new") => Action::New,
        Some(b"--edit") => Action::Edit,
        Some(b"--cat") => Action::Cat,
        Some(b"--which") => Action::Which,
        Some(b"--completion") => Action::Complete,

        Some(b"--") => {
            // Passes all other arguments through to script.
            // Replaces the functionality of `--really`
            args.next();
            Action::Run
        }
        None | Some(_) => Action::Run,
    };

    match action {
        Action::Run => {
            if target.metadata().is_dir() {
                target.directory_help()?;

                if let Some(nonexistent) = args.peek() {
                    eprintln!(
                        "\n`{}` doesn't exist, try creating it with the --new flag",
                        nonexistent.to_string_lossy()
                    );
                }
                Ok(())
            } else {
                let err: MyError = target.exec(args).into();

                Err(err.context(format!("Could not execute the target binary: {}", target)))
            }
        }
        Action::Help => target.help(),
        Action::Cat => target.cat(),
        Action::Edit => target.edit(),
        Action::Which => {
            println!("{}", target);
            Ok(())
        }

        // TODO
        Action::New => unimplemented!(),
        Action::Complete => {
            println!("{}", include_str!("../completion.sh"));
            Ok(())
        }
    }
}
