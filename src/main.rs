extern crate grep_matcher;
extern crate grep_regex;
extern crate grep_searcher;
extern crate ignore;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use exitfailure::ExitFailure;
use failure::ResultExt;
use grep_matcher::{Captures, Matcher};
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

    // Initialize output file handle, which defaults to stdout.
    let mut output_handle = BufWriter::new(match &opt.output {
        Some(path) => Box::new(File::create(path).with_context(|_| "Failed to open output file")?)
            as Box<dyn Write>,
        None => Box::new(io::stdout()) as Box<dyn Write>,
    });

    // This is the regular expression we use to find links.
    let matcher = RegexMatcher::new(r"\[[^\[\]]+\]\(([^\(\)]+)\)")
        .with_context(|_| "Failed to instatiate matcher")?;

    let mut searcher = Searcher::new();

    // We iterator through all files not included in .gitignore.
    let file_iter = Walk::new("./")
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        });

    for x in file_iter {
        let path = x.path();

        write!(output_handle, "\n{}\n", path.display())
            .with_context(|_| "Failed to write output")?;

        searcher.search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                let mut captures = matcher.new_captures().unwrap();
                matcher.captures_iter(line.as_bytes(), &mut captures, |c| {
                    let m = c.get(1).unwrap();
                    let s = line[m].to_string();
                    write!(output_handle, "{}: {}", lnum, s).unwrap();
                    let link = link(s);
                    match verify(&link) {
                        LinkStatus::Reachable(msg) => {
                            write!(output_handle, "  ✓ {}\n", msg).unwrap();
                        }
                        LinkStatus::Unreachable(msg) => {
                            write!(output_handle, "  ✗ {}\n", msg).unwrap();
                        }
                    };
                    true
                })?;
                Ok(true)
            }),
        )?;
    }

    Ok(())
}

fn link(s: String) -> Link {
    if s.starts_with("http") {
        Link::HttpLink(s)
    } else {
        Link::LocalLink(s)
    }
}

enum Link {
    HttpLink(String),
    LocalLink(String),
}

enum LinkStatus {
    Reachable(String),
    Unreachable(String),
}

fn verify(link: &Link) -> LinkStatus {
    match link {
        Link::HttpLink(_) => LinkStatus::Reachable(format!("200")),
        Link::LocalLink(_) => LinkStatus::Unreachable(format!("does not exist")),
    }
}
