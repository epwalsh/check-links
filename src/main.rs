extern crate grep_matcher;
extern crate grep_regex;
extern crate grep_searcher;
extern crate ignore;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use exitfailure::ExitFailure;
use failure::ResultExt;
use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::Walk;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cargo-links",
    about = "Check the links in your crate's documentation.",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Opt {
    #[structopt(parse(from_os_str))]
    /// An optional output file. Default is stdout.
    output: Option<PathBuf>,
}

fn main() -> Result<(), ExitFailure> {
    let opt = Opt::from_args();

    // Initialize output file handle (default to stdout if no path was given).
    let mut output_handle = BufWriter::new(match &opt.output {
        Some(path) => Box::new(File::create(path).with_context(|_| "Failed to open output file")?)
            as Box<Write>,
        None => Box::new(io::stdout()) as Box<Write>,
    });

    let matcher = RegexMatcher::new(r"\[[^\[\]]+\]\([^\(\)]+\)")
        .with_context(|_| "Failed to instatiate matcher")?;
    let mut searcher = Searcher::new();

    for x in Walk::new("./")
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        })
    {
        let path = x.path();
        write!(output_handle, "\n{}\n", path.display())
            .with_context(|_| "Failed to write output")?;
        searcher.search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                // TODO: use `matcher.find_iter` to find all matches on the line.
                let mymatch = matcher.find(line.as_bytes())?.unwrap();
                write!(output_handle, "{}: {}\n", lnum, line[mymatch].to_string())?;
                Ok(true)
            }),
        )?;
    }

    Ok(())
}

// enum Link {
//     HttpLink(String),
//     LocalLink(String),
// }

// enum LinkStatus {
//     Reachable(String),
//     Unreachable(String),
// }

// fn verify(link: &Link) -> LinkStatus {
//     match link {
//         Link::HttpLink(url) => LinkStatus::Reachable(format!("{} (200)", url)),
//         Link::LocalLink(path) => LinkStatus::Unreachable(format!("{} does not exist", path)),
//     }
// }
