//! Manual interactive login. We optionally open a login URL, then pause until
//! the user has logged in by hand and pressed Enter. The session lives in the
//! single reused page, so it persists for the rest of the run.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::info;

use crate::browser::BrowserSession;

pub async fn manual_login(session: &BrowserSession, login_url: Option<&str>) -> Result<()> {
    session.bring_to_front().await;

    if let Some(url) = login_url {
        info!("opening login page: {url}");
        session.page.goto(url).await?;
        let _ = session.page.wait_for_navigation().await;
    }

    let mut stdout = tokio::io::stdout();
    let prompt = if login_url.is_some() {
        "\n>>> Log in in the browser window, then press Enter here to start recording... "
    } else {
        "\n>>> Navigate to your site and log in, then press Enter here to start recording... "
    };
    stdout.write_all(prompt.as_bytes()).await?;
    stdout.flush().await?;

    // Block until the user presses Enter.
    let mut line = String::new();
    let mut reader = BufReader::new(tokio::io::stdin());
    reader.read_line(&mut line).await?;

    info!("login confirmed; starting queue");
    Ok(())
}
