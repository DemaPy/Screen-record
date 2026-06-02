//! Output filename construction: `YYYY-MM-DD_HH-MM-SS_<sanitized-title>.mp4`,
//! limited to 120 characters total. Falls back to a host-based slug when the
//! page title is empty.

use chrono::{DateTime, Local};
use regex::Regex;

const MAX_FILENAME_LEN: usize = 120;
const EXT: &str = ".mp4";

/// Sanitize an arbitrary string into a filename-safe `[a-z0-9-]` slug.
pub fn sanitize_slug(input: &str) -> String {
    let lower = input.to_lowercase();
    // Collapse any run of non-alphanumeric chars into a single dash.
    let re = Regex::new(r"[^a-z0-9]+").unwrap();
    let dashed = re.replace_all(&lower, "-");
    dashed.trim_matches('-').to_string()
}

/// Derive a slug from a URL's host + path, for the temp name and as a title
/// fallback.
pub fn slug_from_url(url: &str) -> String {
    // Strip scheme, keep host+path. Cheap and dependency-free.
    let without_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url);
    let slug = sanitize_slug(without_scheme);
    if slug.is_empty() {
        "page".to_string()
    } else {
        slug
    }
}

/// Build the final filename from a timestamp and page title. Truncates the
/// title portion so the whole name (timestamp + `_` + title + `.mp4`) is
/// within [`MAX_FILENAME_LEN`].
pub fn build_filename(ts: DateTime<Local>, title: &str, url: &str) -> String {
    let stamp = ts.format("%Y-%m-%d_%H-%M-%S").to_string();

    let mut title_slug = sanitize_slug(title);
    if title_slug.is_empty() {
        title_slug = slug_from_url(url);
    }

    // Budget for the title: total - stamp - "_" - ".mp4".
    let fixed = stamp.len() + 1 + EXT.len();
    let budget = MAX_FILENAME_LEN.saturating_sub(fixed);
    if title_slug.len() > budget {
        title_slug.truncate(budget);
        title_slug = title_slug.trim_end_matches('-').to_string();
    }

    format!("{stamp}_{title_slug}{EXT}")
}

/// A temp filename used while recording, before the title is known.
pub fn temp_filename(ts: DateTime<Local>, url: &str) -> String {
    let stamp = ts.format("%Y-%m-%d_%H-%M-%S").to_string();
    let mut slug = slug_from_url(url);
    let fixed = stamp.len() + ".part_".len() + EXT.len();
    let budget = MAX_FILENAME_LEN.saturating_sub(fixed);
    if slug.len() > budget {
        slug.truncate(budget);
    }
    format!("{stamp}.part_{slug}{EXT}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_slug("Hello, World!"), "hello-world");
        assert_eq!(sanitize_slug("  --Trim__This--  "), "trim-this");
        assert_eq!(sanitize_slug("???"), "");
    }

    #[test]
    fn filename_length_capped() {
        let ts = Local::now();
        let long_title = "a".repeat(500);
        let name = build_filename(ts, &long_title, "https://example.com");
        assert!(name.len() <= MAX_FILENAME_LEN, "len was {}", name.len());
        assert!(name.ends_with(".mp4"));
    }

    #[test]
    fn empty_title_falls_back_to_host() {
        let ts = Local::now();
        let name = build_filename(ts, "", "https://example.com/some/path");
        assert!(name.contains("example-com"));
    }
}
