//! User-facing strings (German/English, chosen by the installer via `--lang`).

use std::path::Path;

use crate::{Outcome, Progress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Lang {
        if s.to_ascii_lowercase().starts_with("de") {
            Lang::De
        } else {
            Lang::En
        }
    }
}

pub fn title(l: Lang) -> &'static str {
    match l {
        Lang::De => "HEIC in JPEG umwandeln",
        Lang::En => "Convert HEIC to JPEG",
    }
}

pub fn working(l: Lang) -> &'static str {
    match l {
        Lang::De => "HEIC-Dateien werden in JPEG umgewandelt …",
        Lang::En => "Converting HEIC files to JPEG …",
    }
}

pub fn cancelling(l: Lang) -> &'static str {
    match l {
        Lang::De => "Wird abgebrochen – laufende Dateien werden noch fertig geschrieben …",
        Lang::En => "Cancelling – finishing files in progress …",
    }
}

pub fn progress_line(l: Lang, p: &Progress) -> String {
    match l {
        Lang::De => format!("{} von {} Dateien fertig", p.finished, p.total),
        Lang::En => format!("{} of {} files done", p.finished, p.total),
    }
}

pub fn summary_heading(l: Lang, p: &Progress) -> String {
    if p.total == 0 {
        return match l {
            Lang::De => "Keine HEIC-Dateien gefunden".into(),
            Lang::En => "No HEIC files found".into(),
        };
    }
    match (l, p.converted) {
        (Lang::De, 1) => "1 Datei in JPEG umgewandelt".into(),
        (Lang::De, n) => format!("{n} Dateien in JPEG umgewandelt"),
        (Lang::En, 1) => "1 file converted to JPEG".into(),
        (Lang::En, n) => format!("{n} files converted to JPEG"),
    }
}

pub fn summary_details(l: Lang, p: &Progress) -> String {
    let cancelled = p.total - p.converted - p.failed;
    let mut parts = Vec::new();
    match l {
        Lang::De => {
            if p.failed > 0 {
                parts.push(format!("{} fehlgeschlagen", p.failed));
            }
            if p.skipped > 0 {
                parts.push(format!("{} übersprungen", p.skipped));
            }
            if p.cancelled && cancelled > 0 {
                parts.push(format!("{cancelled} abgebrochen"));
            }
            if parts.is_empty() {
                "Die Originaldateien bleiben unverändert.".into()
            } else {
                parts.join(", ") + ". Die Originaldateien bleiben unverändert."
            }
        }
        Lang::En => {
            if p.failed > 0 {
                parts.push(format!("{} failed", p.failed));
            }
            if p.skipped > 0 {
                parts.push(format!("{} skipped", p.skipped));
            }
            if p.cancelled && cancelled > 0 {
                parts.push(format!("{cancelled} cancelled"));
            }
            if parts.is_empty() {
                "The original files are unchanged.".into()
            } else {
                parts.join(", ") + ". The original files are unchanged."
            }
        }
    }
}

/// One line per problem (failed / skipped / cancelled), for the expandable details.
pub fn problem_lines(l: Lang, results: &[(std::path::PathBuf, Outcome)]) -> Vec<String> {
    let name =
        |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    results
        .iter()
        .filter_map(|(path, outcome)| {
            let reason = match (outcome, l) {
                (Outcome::Converted(_), _) => return None,
                (Outcome::Failed(e), _) => e.clone(),
                (Outcome::NotHeic, Lang::De) => "keine HEIC-Datei, übersprungen".into(),
                (Outcome::NotHeic, Lang::En) => "not a HEIC file, skipped".into(),
                (Outcome::NoHeicInFolder, Lang::De) => "keine HEIC-Dateien in diesem Ordner gefunden".into(),
                (Outcome::NoHeicInFolder, Lang::En) => "no HEIC files found in this folder".into(),
                (Outcome::Cancelled, Lang::De) => "abgebrochen".into(),
                (Outcome::Cancelled, Lang::En) => "cancelled".into(),
            };
            Some(format!("{}: {reason}", name(path)))
        })
        .collect()
}
