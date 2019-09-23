use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use grep_regex::{Error, RegexMatcherBuilder};
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use regex::Regex;

pub struct Link {
    pub file: Arc<PathBuf>,
    pub lnum: usize,
    pub raw: String,
    pub kind: LinkKind,
    pub status: Option<LinkStatus>,
}

pub enum LinkKind {
    Local,
    Http,
}

pub enum LinkStatus {
    Reachable,
    Questionable(String),
    Unreachable(Option<String>),
}

impl Link {
    pub fn new(file: Arc<PathBuf>, lnum: usize, raw: String) -> Self {
        let kind: LinkKind;
        if raw.starts_with("http") {
            kind = LinkKind::Http;
        } else {
            kind = LinkKind::Local;
        }
        Link {
            file,
            lnum,
            raw,
            kind,
            status: None,
        }
    }

    fn split_section(&self) -> (Option<&str>, Option<&str>) {
        lazy_static! {
            static ref SECTION_RE: Regex = Regex::new(r"^(.*)#+([A-Za-z0-9_-]+)$").unwrap();
        }
        match SECTION_RE.captures(&self.raw[..]) {
            Some(caps) => {
                let section = caps.get(2).unwrap().as_str();
                let base = caps.get(1).unwrap().as_str();
                if base == "" {
                    (None, Some(section))
                } else {
                    (Some(base), Some(section))
                }
            }
            None => (Some(&self.raw[..]), None),
        }
    }

    fn _verify(&self, http_client: Arc<reqwest::Client>) -> LinkStatus {
        match self.kind {
            LinkKind::Http => {
                match http_client.head(&self.raw[..]).send() {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        match status {
                            200 => LinkStatus::Reachable,
                            // the resource exists but may require logging in.
                            401 => {
                                LinkStatus::Questionable(format!("received status code {}", status))
                            }
                            // ^ same
                            403 => {
                                LinkStatus::Questionable(format!("received status code {}", status))
                            }
                            // HEAD method not allowed.
                            405 => {
                                LinkStatus::Questionable(format!("received status code {}", status))
                            }
                            // resource exits, but our 'Accept-' header may not match what the server can provide.// resource exits, but our 'Accept-' header may not match what the server can provide.
                            406 => {
                                LinkStatus::Questionable(format!("received status code {}", status))
                            }
                            _ => LinkStatus::Unreachable(Some(format!(
                                "received status code {}",
                                status
                            ))),
                        }
                    }
                    Err(e) => {
                        if e.is_timeout() {
                            LinkStatus::Unreachable(Some(String::from("timeout error")))
                        } else {
                            match e.status() {
                                Some(status) => LinkStatus::Unreachable(Some(format!(
                                    "received status code {}",
                                    status
                                ))),
                                None => LinkStatus::Unreachable(None),
                            }
                        }
                    }
                }
            }
            LinkKind::Local => {
                let dir = match self.file.as_ref().parent() {
                    Some(d) => d,
                    None => Path::new("./"),
                };
                let (base, section) = self.split_section();
                match section {
                    // If no section, just check that base exists.
                    None => match base {
                        Some(b) => {
                            let full_path = dir.join(Path::new(b));
                            match full_path.exists() {
                                true => LinkStatus::Reachable,
                                false => LinkStatus::Unreachable(None),
                            }
                        }
                        None => LinkStatus::Unreachable(None),
                    },
                    // But if there is a section...
                    Some(s) => match base {
                        Some(b) => {
                            let full_path = dir.join(Path::new(b));
                            match full_path.exists() {
                                true => match self.find_section(&full_path, s) {
                                    Ok(true) => LinkStatus::Reachable,
                                    Ok(false) => LinkStatus::Questionable(format!(
                                        "failed to resolve section #{}",
                                        s
                                    )),
                                    Err(e) => LinkStatus::Questionable(format!(
                                        "failed to resolve section #{} {:?}",
                                        s, e
                                    )),
                                },
                                false => LinkStatus::Unreachable(None),
                            }
                        }
                        None => match self.find_section(self.file.as_ref(), s) {
                            Ok(true) => LinkStatus::Reachable,
                            Ok(false) => LinkStatus::Questionable(format!(
                                "failed to resolve section #{}",
                                s
                            )),
                            Err(e) => LinkStatus::Questionable(format!(
                                "failed to find section #{} {:?}",
                                s, e
                            )),
                        },
                    },
                }
            }
        }
    }

    pub fn verify(&mut self, http_client: Arc<reqwest::Client>) {
        self.status = Some(self._verify(http_client));
    }

    pub fn find_section(&self, path: &Path, section: &str) -> Result<bool, Error> {
        let mut searcher = Searcher::new();
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build(&section.replace("-", " ")[..])?;
        let mut found: bool = false;
        searcher
            .search_path(
                &matcher,
                path,
                UTF8(|_, _| {
                    found = true;
                    Ok(true)
                }),
            )
            .unwrap();
        Ok(found)
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [line {}]: {}",
            self.file.as_ref().display(),
            self.lnum,
            self.raw
        )
    }
}
