# check-links

[![Build Status](https://travis-ci.org/epwalsh/check-links.svg?branch=master)](https://travis-ci.org/epwalsh/check-links) [![Latest version](https://img.shields.io/crates/v/check-links.svg)](https://crates.io/crates/check-links) ![License](https://img.shields.io/crates/l/check-links.svg)

A command-line utility for finding stale links in your crate's documentation.

Run `check-links` in the root of your project to recursively search for bad links across Markdown files and documentation comments in source files.

## Installing

#### From cargo:

```
cargo install check-links
```

#### From source:

```
git clone https://github.com/epwalsh/check-links.git
cd check-links
make release
ln -s [current directory]/target/release/check-links ~/bin/
```

## A note about spamming the internet

This script can be a useful addition to your CI pipeline to catch stale documentation, but if you have a ton of HTTP links in your project you may want to avoid running `check-links` too often, as your CI server could end up being blocked or rate-limited by certain hosts.
