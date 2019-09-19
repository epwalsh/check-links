use std::sync::mpsc::channel;
use std::sync::Arc;

use exitfailure::ExitFailure;
use grep_searcher::Searcher;
use ignore::Walk;
use structopt::StructOpt;
use threadpool::ThreadPool;

mod doc_file;
mod link;
mod log;

use doc_file::DocFile;
use link::{Link, LinkStatus};
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

    // Initialize thread pool and channel.
    let pool = ThreadPool::new(opt.concurrency);
    let (tx, rx) = channel();

    // We'll use a single HTTP client across threads.
    let http_client = Arc::new(reqwest::Client::new());

    let doc_files = vec![
        // Rust source files.
        DocFile::new(vec!["*.rs"], r"\[[^\[\]]+\]\(([^\(\)]+)\)"),
        // Extra markdown files.
        DocFile::new(vec!["*.md"], r"\[[^\[\]]+\]\(([^\(\)]+)\)"),
    ];

    // We iterator through all rust and markdown files not included in your .gitignore.
    let file_iter = Walk::new("./")
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        })
        .map(|x| x.into_path());

    let mut n_links = 0;
    let mut searcher = Searcher::new();

    for path in file_iter {
        for doc_file in &doc_files {
            if doc_file.is_match(&path) {
                logger.debug(&format!("Searching {}", path.display())[..])?;

                let path_arc = Arc::new(path);

                doc_file.iter_links(&mut searcher, &path_arc, |lnum, mat| {
                    n_links += 1;
                    let mut link = Link::new(Arc::clone(&path_arc), lnum as usize, mat);
                    let tx = tx.clone();
                    let http_client = http_client.clone();
                    pool.execute(move || {
                        link.verify(http_client);
                        tx.send(link).unwrap();
                    });
                    true
                })?;

                break;
            }
        }
    }

    let mut n_bad_links = 0;
    for link in rx.iter().take(n_links) {
        match link.status.as_ref().unwrap() {
            LinkStatus::Reachable => {
                logger.info(&format!("✓ {}", link)[..])?;
            }
            LinkStatus::Questionable(reason) => {
                logger.warn(&format!("✓ {} ({})", link, reason)[..])?
            }
            LinkStatus::Unreachable(reason) => {
                n_bad_links += 1;
                match reason {
                    Some(s) => logger.error(&format!("✗ {} ({})", link, s)[..])?,
                    None => logger.error(&format!("✗ {}", link)[..])?,
                };
            }
        };
    }

    if n_bad_links > 0 {
        logger.error(&format!("Found {} bad links", n_bad_links)[..])?;
        std::process::exit(1);
    }

    Ok(())
}
