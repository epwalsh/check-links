use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::Arc;

use exitfailure::ExitFailure;
use grep_matcher::{Captures, Matcher};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::Walk;
use structopt::StructOpt;
use threadpool::ThreadPool;

mod log;

use log::Logger;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cargo-links",
    about = "Check the links in your crate's documentation.",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Opt {
    /// Set the number of threads.
    #[structopt(short = "c", long = "concurrency", default_value = "10")]
    concurrency: usize,

    /// Verbose mode (-v, -vv, -vvv, etc).
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,

    /// Don't log in color.
    #[structopt(long = "no-color")]
    no_color: bool,
}

fn main() -> Result<(), ExitFailure> {
    let opt = Opt::from_args();
    let mut logger = Logger::default(opt.verbose, !opt.no_color);
    logger.debug(&format!("{:?}", opt)[..])?;

    // This is the regular expression we use to find links.
    let matcher = RegexMatcher::new(r"\[[^\[\]]+\]\(([^\(\)]+)\)").unwrap();

    let mut searcher = Searcher::new();

    // Initialize thread pool and channel.
    let pool = ThreadPool::new(opt.concurrency);
    let (tx, rx) = channel();

    // We'll use a single HTTP client across threads.
    let http_client = Arc::new(reqwest::Client::new());

    // We iterator through all files not included in .gitignore.
    let file_iter = Walk::new("./")
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        });

    let mut n_links = 0;
    for x in file_iter {
        let path = x.path();
        let path_str = path.to_str();
        if let None = path_str {
            // File path is not valid unicode, just skip.
            logger.warn(
                &format!(
                    "Filename is not valid unicode, skipping: {}",
                    path.display()
                )[..],
            )?;
            continue;
        }
        let path_str = path_str.unwrap();

        logger.debug(&format!("Searching {}", path.display())[..])?;

        searcher.search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                let mut captures = matcher.new_captures().unwrap();
                matcher.captures_iter(line.as_bytes(), &mut captures, |c| {
                    n_links += 1;
                    let m = c.get(1).unwrap();
                    let raw = line[m].to_string();

                    let mut link = Link::new(String::from(path_str), lnum as usize, raw);

                    let tx = tx.clone();
                    let http_client = http_client.clone();
                    pool.execute(move || {
                        link.verify(http_client);
                        tx.send(link).unwrap();
                    });

                    true
                })?;

                Ok(true)
            }),
        )?;
    }

    let mut n_bad_links = 0;
    for link in rx.iter().take(n_links) {
        match link.status.unwrap() {
            LinkStatus::Reachable => {
                logger.info(&format!("✓ {} {}: {}", link.file, link.lnum, link.raw)[..])?;
            }
            LinkStatus::Unreachable(reason) => {
                n_bad_links += 1;
                logger.error(
                    &format!("✗ {} {}: {} ({})", link.file, link.lnum, link.raw, reason)[..],
                )?;
            }
        };
    }

    if n_bad_links > 0 {
        logger.error(&format!("Found {} bad links", n_bad_links)[..])?;
        std::process::exit(1);
    }

    Ok(())
}

struct Link {
    file: String,
    lnum: usize,
    raw: String,
    status: Option<LinkStatus>,
}

impl Link {
    fn new(file: String, lnum: usize, raw: String) -> Self {
        Link {
            file,
            lnum,
            raw,
            status: None,
        }
    }

    fn _verify(&self, http_client: Arc<reqwest::Client>) -> LinkStatus {
        if self.raw.starts_with("http") {
            match http_client.head(&self.raw[..]).send() {
                Ok(response) => {
                    let status = response.status().as_u16();
                    match status {
                        200 => LinkStatus::Reachable,
                        _ => LinkStatus::Unreachable(format!("received status code {}", status)),
                    }
                }
                Err(e) => {
                    if e.is_timeout() {
                        LinkStatus::Unreachable(String::from("timeout error"))
                    } else {
                        match e.status() {
                            Some(status) => {
                                LinkStatus::Unreachable(format!("received status code {}", status))
                            }
                            None => LinkStatus::Unreachable(String::from("unknown")),
                        }
                    }
                }
            }
        } else {
            if Path::new(&self.raw[..]).exists() {
                LinkStatus::Reachable
            } else {
                LinkStatus::Unreachable(String::from("does not exist"))
            }
        }
    }

    fn verify(&mut self, http_client: Arc<reqwest::Client>) {
        self.status = Some(self._verify(http_client));
    }
}

enum LinkStatus {
    Reachable,
    Unreachable(String),
}
