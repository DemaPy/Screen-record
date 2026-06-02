//! Per-page browser console capture, used by failure recovery. We subscribe to
//! the CDP `Runtime.consoleAPICalled` and `Runtime.exceptionThrown` events and
//! serialize each event to JSON (sidesteps brittle field access). The buffer is
//! reset per URL so it never grows across the run.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use chromiumoxide::cdp::js_protocol::runtime::{EventConsoleApiCalled, EventExceptionThrown};
use chromiumoxide::page::Page;
use futures::StreamExt;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct ConsoleCapture {
    buf: Arc<Mutex<Vec<String>>>,
    _tasks: Arc<Vec<JoinHandle<()>>>,
}

impl ConsoleCapture {
    pub async fn attach(page: &Page) -> Result<Self> {
        let buf = Arc::new(Mutex::new(Vec::new()));

        let mut console = page.event_listener::<EventConsoleApiCalled>().await?;
        let cb = Arc::clone(&buf);
        let t1 = tokio::spawn(async move {
            while let Some(ev) = console.next().await {
                if let Ok(s) = serde_json::to_string(&*ev) {
                    if let Ok(mut g) = cb.lock() {
                        g.push(format!("console: {s}"));
                    }
                }
            }
        });

        let mut exceptions = page.event_listener::<EventExceptionThrown>().await?;
        let eb = Arc::clone(&buf);
        let t2 = tokio::spawn(async move {
            while let Some(ev) = exceptions.next().await {
                if let Ok(s) = serde_json::to_string(&*ev) {
                    if let Ok(mut g) = eb.lock() {
                        g.push(format!("exception: {s}"));
                    }
                }
            }
        });

        Ok(Self {
            buf,
            _tasks: Arc::new(vec![t1, t2]),
        })
    }

    /// Clear the buffer (call at the start of each URL).
    pub fn reset(&self) {
        if let Ok(mut g) = self.buf.lock() {
            g.clear();
        }
    }

    /// Current captured lines.
    pub fn snapshot(&self) -> Vec<String> {
        self.buf.lock().map(|g| g.clone()).unwrap_or_default()
    }
}
