use clap::{Arg, ArgAction};

pub const VERBOSE: &str = "verbose";

pub fn show_tags() -> Arg {
    Arg::new("show_tags")
        .long("show-tags")
        .action(ArgAction::SetTrue)
        .help("Also show tags")
}

pub fn delete(force: bool) -> Arg {
    let short = if force { 'D' } else { 'd' };
    Arg::new("delete").short(short)
}

pub fn verbose() -> Arg {
    Arg::new(VERBOSE)
        .short('v')
        .long("verbose")
        .action(ArgAction::Count)
        .help(
            "Set verbosity of output. \
            Verbosity increases with number of occurrences.",
        )
}
