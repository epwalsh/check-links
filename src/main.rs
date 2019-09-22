use std::sync::mpsc::channel;
use std::sync::Arc;

use exitfailure::ExitFailure;
use ignore::Walk;
use structopt::StructOpt;
use threadpool::ThreadPool;

mod doc_file;
mod link;
mod log;

use doc_file::DocFile;
use link::LinkStatus;
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

    // Initialize thread pool and channel. Each link to check will get its own copy
    // of the transmitter `tx`. When the link is verified we'll send the results through
    // the channel to the receiver `rx`. Then we gather all the results and log them
    // to the terminal.
    let pool = ThreadPool::new(opt.concurrency);
    let (tx, rx) = channel();

    // We'll use a single HTTP client across threads.
    let http_client = Arc::new(reqwest::Client::new());

    // We'll search all Rust and Markdown files.
    let doc_files = vec![
        // Rust files.
        DocFile::new(
            vec!["*.rs"],
            r"^\s*(///|//!).*\[[^\[\]]+\]\(([^\(\)]+)\)",
            2,
        ),
        // Markdown files.
        DocFile::new(vec!["*.md"], r"\[[^\[\]]+\]\(([^\(\)]+)\)", 1),
    ];

    // Build file iterator.
    // We iterator through all non-hidden Rust and Markdown files not included in a .gitignore.
    let file_iter = Walk::new("./")
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        })
        .map(|x| x.into_path());

    // Keep track of the total number of links so we know how many the receiver `rx`
    // should be expecting.
    let mut n_links = 0;

    // Now iter through all files in our `file_iter` and check if they match one of
    // the doc files.
    for path in file_iter {
        for doc_file in &doc_files {
            if doc_file.is_match(&path) {
                logger.debug(&format!("Searching {}", path.display())[..])?;

                let path_arc = Arc::new(path);

                // Search for links in the file. For each link found, we send a closure
                // to the thread pool that will verify the link and report the results
                // to the channel.
                doc_file.iter_links(&path_arc, |mut link| {
                    n_links += 1;
                    let tx = tx.clone();
                    let http_client = http_client.clone();
                    pool.execute(move || {
                        link.verify(http_client);
                        tx.send(link).unwrap();
                    });
                })?;

                break;
            }
        }
    }

    // Now loop through all the links we found and log the results to the terminal.
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
        // Exit with an error code if any bad links were found.
        logger.error(&format!("Found {} bad links", n_bad_links)[..])?;
        std::process::exit(1);
    }

    Ok(())
}
