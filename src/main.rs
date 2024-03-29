#[macro_use]
extern crate lazy_static;

use std::sync::Arc;
use std::time::Duration;

use exitfailure::ExitFailure;
use ignore::WalkBuilder;
use structopt::StructOpt;
use tokio::sync::mpsc::channel;

mod doc_file;
mod link;
mod log;

use doc_file::DocFile;
use link::LinkStatus;
use log::Logger;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "check-links",
    about = "Check the links in your crate's documentation.",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Opt {
    /// Verbose mode (-v, -vv, -vvv, etc)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,

    /// Don't log in color
    #[structopt(long = "no-color")]
    no_color: bool,

    /// Set the maximum directory depth to recurse
    #[structopt(short = "d", long = "depth")]
    depth: Option<usize>,

    /// Specify the timeout for HTTP requests
    #[structopt(short = "t", long = "timeout", default_value = "3")]
    timeout: u64,
}

fn maybe_pluralize(n: u32) -> &'static str {
    match n {
        1 => "",
        _ => "s",
    }
}

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<(), ExitFailure> {
    let opt = Opt::from_args();
    let mut logger = Logger::default(opt.verbose, !opt.no_color);
    logger.debug(&format!("{:?}", opt)[..])?;

    // Initialize a MPSC channel. Each link to check will get its own copy
    // of the transmitter `tx`. When the link is verified we'll send the results through
    // the channel to the receiver `rx`. Then we gather all the results and log them
    // to the terminal.
    let (tx, mut rx) = channel(100);

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
    let file_iter = WalkBuilder::new("./")
        .max_depth(opt.depth)
        .build()
        .filter_map(Result::ok)
        .filter(|x| match x.file_type() {
            Some(file_type) => file_type.is_file(),
            None => false,
        })
        .map(|x| x.into_path());

    // Keep track of the total number of links so we know how many the receiver `rx`
    // should be expecting.
    let mut n_links = 0u32;

    // Configure HTTP client.
    let http_client = Arc::new(
        isahc::HttpClient::builder()
            .timeout(Duration::from_secs(opt.timeout))
            .connect_timeout(Duration::from_secs(opt.timeout))
            .build()?,
    );

    // Now iter through all files in our `file_iter` and check if they match one of
    // the doc files.
    for path in file_iter {
        for doc_file in &doc_files {
            if doc_file.is_match(&path) {
                logger.debug(&format!("Searching {}", path.display())[..])?;

                // Search for links in the file. For each link found, we spawn a task
                // that will verify the link and report the results to the channel.
                doc_file.iter_links(&path, |mut link| {
                    n_links += 1;
                    let mut tx = tx.clone();
                    let http_client = http_client.clone();
                    tokio::spawn(async move {
                        link.verify(http_client).await;
                        if tx.send(link).await.is_err() {
                            std::process::exit(1);
                        };
                    });
                })?;

                break;
            } else {
                logger.debug(&format!("Ignoring {}", path.display())[..])?;
            }
        }
    }

    // Drop transmitter so that the receiver knows it is finished.
    drop(tx);

    // Now loop through all the links we found and log the results to the terminal.
    let mut n_errors = 0u32;
    let mut n_warnings = 0u32;
    while let Some(link) = rx.recv().await {
        match link.status.as_ref().unwrap() {
            LinkStatus::Reachable => {
                logger.info(&format!("✓ {}", link)[..])?;
            }
            LinkStatus::Questionable(reason) => {
                n_warnings += 1;
                logger.warn(&format!("✗ {}\n        ► {}", link, reason)[..])?;
            }
            LinkStatus::Unreachable(reason) => {
                n_errors += 1;
                match reason {
                    Some(s) => logger.error(&format!("✗ {}\n        ► {}", link, s)[..])?,
                    None => logger.error(&format!("✗ {}", link)[..])?,
                };
            }
        };
    }

    if n_links == 0 {
        logger.info("No links found")?;
    } else if n_errors > 0 {
        // Exit with an error code if any bad links were found.
        logger.error(&format!(
            "{} error{}, {} warning{} out of {} link{} found",
            n_errors,
            maybe_pluralize(n_errors),
            n_warnings,
            maybe_pluralize(n_warnings),
            n_links,
            maybe_pluralize(n_links)
        ))?;
        std::process::exit(1);
    } else {
        logger.info(&format!(
            "{} error{}, {} warning{} out of {} link{} found",
            n_errors,
            maybe_pluralize(n_errors),
            n_warnings,
            maybe_pluralize(n_warnings),
            n_links,
            maybe_pluralize(n_links)
        ))?;
    }

    Ok(())
}
