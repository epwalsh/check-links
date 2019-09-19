use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    pub fn new<'a>(file: Arc<PathBuf>, lnum: usize, raw: String) -> Self {
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
                if dir.join(Path::new(&self.raw[..])).exists() {
                    LinkStatus::Reachable
                } else {
                    LinkStatus::Unreachable(None)
                }
            }
        }
    }

    pub fn verify(&mut self, http_client: Arc<reqwest::Client>) {
        self.status = Some(self._verify(http_client));
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.file.as_ref().display(),
            self.lnum,
            self.raw
        )
    }
}
