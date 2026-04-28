mod target;
use target::Target;

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context;
type MyError = anyhow::Error;

const BLUE_TEXT: &str = "\x1b[34m";
const BRIGHT_YELLOW_TEXT: &str = "\x1b[93m";
const DEFAULT_TEXT: &str = "\x1b[0m";

const SCRIPT_TEMPLATE: &str = "#!/usr/bin/env bash\n\nset -euo pipefail\n";

fn user_friendly_panic_report(info: &std::panic::PanicHookInfo<'_>) {
    eprintln!("{}\n", info);

    let bt = std::backtrace::Backtrace::force_capture();
    eprint!("Backtrace:\n{bt}");

    eprint!(
        "
Sorry about that, this program appears to have run into a small spot of bother.

I consider every `panic!` that happens to be a bug and you've found one!
I'd be grateful if you were to open an issue at https://github.com/syberant/sdr and include:
- the above panic message and backtrace
- what you were trying to do/any relevant circumstances
- operating system/distro (`grep -E '^(NAME|VERSION)=' /etc/os-release`)
- kernel version (`uname -a`)
"
    );
}

/// The action we are supposed to carry out.
enum Action {
    Run,
    Help,
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
    std::env::var_os("SD_EDITOR")
        .or_else(|| std::env::var_os("VISUAL"))
        .or_else(|| std::env::var_os("EDITOR"))
        .unwrap_or(OsString::from("vi"))
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

fn parse_help<P: AsRef<std::path::Path>>(path: P) -> Option<impl Iterator<Item = String>> {
    use std::io::BufRead;

    let f = std::fs::File::open(path).ok()?;
    let f = std::io::BufReader::new(f);

    let help_text = f
        .lines()
        // Stop processing lines after the first error
        .take_while(|l| l.is_ok())
        .map(|l| l.expect("This should be impossible, please submit a bug report."))
        // Skip the `#!` (binfmt) line
        .skip(1)
        // Skip non-comment lines and pick the first contiguous block of comments
        .skip_while(|line| line.chars().next() != Some('#'))
        // Take only the first block of comments
        .take_while(|line| line.chars().next() == Some('#'))
        .map(|line| {
            // Remove the `#` and whitespace that follows it.
            line.get(1..).unwrap_or("").trim_start().to_string()
        });

    // Return `None` if our iterator is empty
    let mut help_text = help_text.peekable();
    if help_text.peek().is_none() {
        None
    } else {
        Some(help_text)
    }
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
    std::panic::set_hook(Box::new(user_friendly_panic_report));

    let (target, args) = parse_target()?;

    let mut args = args.peekable();
    let next_arg = args.peek().map(|s| s.as_encoded_bytes());
    let arg_count = std::env::args_os().count();

    let action = match (arg_count, next_arg) {
        // These flags only work when there is only 1 argument.
        (2, Some(b"--help")) => {
            print!("{}", include_str!("../help"));
            return Ok(());
        }
        (2, Some(b"--version")) => {
            // Get version from cargo metadata at build time
            // NOTE: Maybe git revision too? Requires a build script.
            let version = option_env!("CARGO_PKG_VERSION")
                .unwrap_or("? (not built by cargo, unknown version)");
            println!("sdr v{}", version);
            return Ok(());
        }
        (2, Some(b"--completion-bash")) => Action::Complete,

        (_, Some(b"--help")) => Action::Help,
        (_, Some(b"--edit")) => Action::Edit,
        (_, Some(b"--cat")) => Action::Cat,
        (_, Some(b"--which")) => Action::Which,

        (_, Some(b"--")) => {
            // Passes all other arguments through to script.
            // Replaces the functionality of `--really`
            args.next();
            Action::Run
        }
        (_, None | Some(_)) => Action::Run,
    };

    match action {
        Action::Run => {
            if target.metadata().is_dir() {
                if let Some(nonexistent) = args.next() {
                    // FIXME: Allow creating a new nested script a la `mkdir -p`
                    if args.peek() == Some(&OsString::from("--new")) {
                        target.create_new(&nonexistent)?;

                        return Ok(());
                    } else {
                        eprintln!(
                            "\n`{}` doesn't exist, try creating it with the --new flag\n",
                            nonexistent.to_string_lossy()
                        );
                    }
                }

                target.help()?;

                Ok(())
            } else {
                let err: MyError = target.exec(args).into();

                Err(err.context(format!("Could not execute the target binary: {}", target)))
            }
        }
        Action::Help => target.help(),
        Action::Cat => target.cat(),
        Action::Edit => Target::edit(target),
        Action::Which => {
            println!("{}", target);
            Ok(())
        }

        Action::Complete => {
            println!("{}", include_str!("../completion.sh"));
            Ok(())
        }
    }
}
