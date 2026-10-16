//! heic-convert.exe – converts HEIC/HEIF files (or all HEIC files in folders) to JPEG.
//!
//!   heic-convert [--lang=de|en] [--quiet] <file|folder>...
//!
//! Without --quiet (Explorer context menu): one progress window collects all files of a
//! multi-selection, because Explorer starts one process per selected file.
//! With --quiet: no UI, one line per file on stdout, exit code 0 = ok, 1 = failures, 2 = usage.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod gui;

use std::path::PathBuf;

use heic_convert::text::{self, Lang};
use heic_convert::{convert_all, Outcome};

pub struct Args {
    pub lang: Lang,
    pub quiet: bool,
    pub inputs: Vec<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args { lang: Lang::En, quiet: false, inputs: Vec::new() };
    for a in std::env::args_os().skip(1) {
        match a.to_str() {
            Some("--quiet") | Some("-q") => args.quiet = true,
            Some(s) if s.starts_with("--lang=") => args.lang = Lang::parse(&s[7..]),
            _ => args.inputs.push(std::path::absolute(&a).unwrap_or_else(|_| PathBuf::from(&a))),
        }
    }
    args
}

fn run_quiet(args: &Args) -> i32 {
    if args.inputs.is_empty() {
        eprintln!("usage: heic-convert [--lang=de|en] [--quiet] <file|folder>...");
        return 2;
    }
    let batch = convert_all(&args.inputs);
    for (src, outcome) in batch.results() {
        match outcome {
            Outcome::Converted(dst) => println!("OK    {} -> {}", src.display(), dst.display()),
            Outcome::Failed(e) => println!("FAIL  {}: {e}", src.display()),
            Outcome::NotHeic => println!("SKIP  {}: not a HEIC file", src.display()),
            Outcome::NoHeicInFolder => println!("SKIP  {}: no HEIC files in folder", src.display()),
            Outcome::Cancelled => println!("SKIP  {}: cancelled", src.display()),
        }
    }
    let p = batch.progress();
    println!("{} – {}", text::summary_heading(args.lang, &p), text::summary_details(args.lang, &p));
    if p.failed > 0 {
        1
    } else {
        0
    }
}

fn main() {
    let args = parse_args();
    #[cfg(windows)]
    {
        gui::lower_priority();
        if args.quiet {
            gui::attach_parent_console();
        } else {
            std::process::exit(gui::run(args));
        }
    }
    std::process::exit(run_quiet(&args));
}
