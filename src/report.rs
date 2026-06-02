//! Run report. We accumulate one record per URL and write `report.json` after
//! every URL (crash-safe) plus a final summary.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct UrlResult {
    pub url: String,
    pub title: Option<String>,
    pub output_file: Option<String>,
    pub status: Status,
    pub started_at: String,
    pub ended_at: String,
    pub duration_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console_log_path: Option<String>,
    /// True if the recording had to be force-killed (file may be truncated).
    pub forced_stop: bool,
}

#[derive(Debug, Serialize)]
struct Summary {
    total: usize,
    succeeded: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct Report<'a> {
    summary: Summary,
    results: &'a [UrlResult],
}

pub struct Reporter {
    path: PathBuf,
    results: Vec<UrlResult>,
}

impl Reporter {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            path: output_dir.join("report.json"),
            results: Vec::new(),
        }
    }

    /// Append a result and persist the whole report immediately.
    pub fn record(&mut self, result: UrlResult) -> Result<()> {
        self.results.push(result);
        self.flush()
    }

    pub fn flush(&self) -> Result<()> {
        let succeeded = self
            .results
            .iter()
            .filter(|r| r.status == Status::Success)
            .count();
        let report = Report {
            summary: Summary {
                total: self.results.len(),
                succeeded,
                failed: self.results.len() - succeeded,
            },
            results: &self.results,
        };
        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(&self.path, json)
            .with_context(|| format!("writing report {}", self.path.display()))?;
        Ok(())
    }
}
