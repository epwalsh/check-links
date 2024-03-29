use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_matcher::{Captures, Matcher};
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;

use crate::link::Link;

pub struct DocFile {
    glob_set: GlobSet,
    pub link_matcher: RegexMatcher,
    match_group: usize,
}

impl DocFile {
    pub fn new(globs: Vec<&str>, link_pattern: &str, match_group: usize) -> Self {
        let mut glob_builder = GlobSetBuilder::new();
        for glob in globs {
            glob_builder.add(Glob::new(glob).unwrap());
        }
        let glob_set = glob_builder.build().unwrap();
        let link_matcher = RegexMatcher::new(link_pattern).unwrap();
        DocFile {
            glob_set,
            link_matcher,
            match_group,
        }
    }

    pub fn is_match<P>(&self, p: P) -> bool
    where
        P: AsRef<Path>,
    {
        self.glob_set.is_match(p)
    }

    pub fn iter_links<F>(&self, p: &PathBuf, mut f: F) -> Result<(), io::Error>
    where
        F: FnMut(Link),
    {
        let mut searcher = Searcher::new();
        searcher.search_path(
            &self.link_matcher,
            p,
            UTF8(|lnum, line| {
                let mut captures = self.link_matcher.new_captures().unwrap();
                self.link_matcher
                    .captures_iter(line.as_bytes(), &mut captures, |c| {
                        let mat = c.get(self.match_group).unwrap();
                        let mat = line[mat].to_string();
                        let link = Link::new(p.clone(), lnum as usize, mat);
                        f(link);
                        true
                    })?;
                Ok(true)
            }),
        )
    }
}
