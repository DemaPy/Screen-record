//! Browser session management. We launch a single headful Chromium, maximized,
//! and reuse one page for login plus every queued URL so the logged-in session
//! persists for the whole run.

use std::path::PathBuf;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::error::SetupError;

/// chromiumoxide's built-in `DEFAULT_ARGS`, minus the two automation tells:
/// `--enable-automation` (shows the "controlled by automated test software"
/// infobar and sets `navigator.webdriver = true`) and
/// `--enable-blink-features=IdleDetection`. We disable the crate's defaults and
/// re-add this curated subset so the browser doesn't advertise that it's
/// script-controlled. CDP still works — that rides on `--remote-debugging-port`,
/// which is added separately. See also the `webdriver` patch in `launch`.
const CURATED_DEFAULT_ARGS: &[&str] = &[
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-features=TranslateUI",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--password-store=basic",
    "--use-mock-keychain",
    "--lang=en_US",
];

/// Injected into every new document. `--disable-blink-features=Automation`
/// `Controlled` is *not* sufficient on current Chrome (verified: `navigator.`
/// `webdriver` stays `true`), so we additionally hide the property here. Applied
/// via `addScriptToEvaluateOnNewDocument`, it survives navigations, so a single
/// injection covers every URL in the queue on our one reused page.
const WEBDRIVER_PATCH: &str =
    "Object.defineProperty(navigator, 'webdriver', { get: () => undefined });";

pub struct BrowserSession {
    /// Held to keep the CDP connection alive; dropping it closes the browser.
    pub browser: Browser,
    /// The single reused page (carries cookies/session across navigations).
    pub page: Page,
    /// The Chromium `.app` bundle, used to raise it to the foreground.
    app_bundle: Option<PathBuf>,
    /// Ephemeral profile dir for this run; removed on close.
    profile_dir: PathBuf,
    handler: Option<JoinHandle<()>>,
}

/// Find the enclosing `.app` bundle for an executable path
/// (e.g. `…/Chromium.app/Contents/MacOS/Chromium` → `…/Chromium.app`).
fn app_bundle_of(exe: &std::path::Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
        .map(|p| p.to_path_buf())
}

impl BrowserSession {
    /// Launch Chromium (headful, maximized) and open a blank reusable page.
    pub async fn launch(chrome_path: PathBuf) -> Result<Self, SetupError> {
        let app_bundle = app_bundle_of(&chrome_path);

        // Use a unique, ephemeral profile per run. chromiumoxide otherwise
        // reuses a fixed-name temp profile, which session-restores the previous
        // run's tabs. A fresh dir gives a clean window and isolates concurrent
        // runs; the in-run login session still persists across the queue.
        let profile_dir =
            std::env::temp_dir().join(format!("recorder-profile-{}", std::process::id()));

        // Disable chromiumoxide's DEFAULT_ARGS (which include the automation
        // tells) and re-add a curated subset — see CURATED_DEFAULT_ARGS.
        let mut builder = BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .user_data_dir(&profile_dir)
            .with_head()
            .disable_default_args();
        for arg in CURATED_DEFAULT_ARGS {
            builder = builder.arg(*arg);
        }
        let config = builder
            // Fill the captured display. 480p output is only legible because the
            // maximized window covers the frame — do not run windowed.
            .arg("--start-maximized")
            .arg("--window-position=0,0")
            .arg("--no-default-browser-check")
            .arg("--disable-session-crashed-bubble")
            .arg("--disable-infobars")
            .arg("--disable-blink-features=AutomationControlled")
            // Use the real window size, not an emulated viewport.
            .viewport(None)
            .build()
            .map_err(SetupError::BrowserLaunch)?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| SetupError::BrowserLaunch(e.to_string()))?;

        // The handler stream must be driven for the browser to function.
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| SetupError::BrowserLaunch(e.to_string()))?;

        // Hide the `navigator.webdriver` automation tell on every document.
        use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
        if let Err(e) = page
            .execute(AddScriptToEvaluateOnNewDocumentParams::new(WEBDRIVER_PATCH))
            .await
        {
            warn!("could not install webdriver patch: {e}");
        }

        let session = Self {
            browser,
            page,
            app_bundle,
            profile_dir,
            handler: Some(handler_task),
        };
        session.maximize().await;
        info!("browser launched");
        Ok(session)
    }

    /// Make the window fill the screen. On macOS the `Maximized` window state
    /// (and `--start-maximized`) are unreliable, so we read the usable screen
    /// rect from the renderer and set explicit window bounds — this reliably
    /// fills the captured frame so the 480p downscale stays legible.
    async fn maximize(&self) {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams, WindowState,
        };

        #[derive(serde::Deserialize)]
        struct ScreenRect {
            left: i64,
            top: i64,
            w: i64,
            h: i64,
        }

        let rect: ScreenRect = match self
            .page
            .evaluate(
                "({ left: window.screen.availLeft|0, top: window.screen.availTop|0, \
                    w: window.screen.availWidth, h: window.screen.availHeight })",
            )
            .await
            .and_then(|r| r.into_value().map_err(Into::into))
        {
            Ok(r) => r,
            Err(e) => {
                warn!("could not read screen size for maximize: {e}");
                return;
            }
        };

        let win = match self.page.execute(GetWindowForTargetParams::default()).await {
            Ok(w) => w,
            Err(e) => {
                warn!("could not get window for maximize: {e}");
                return;
            }
        };
        let window_id = win.result.window_id;

        // Bounds only apply in the Normal state; leave maximized/fullscreen first.
        let normal = Bounds::builder().window_state(WindowState::Normal).build();
        let _ = self
            .page
            .execute(SetWindowBoundsParams::new(window_id, normal))
            .await;

        let bounds = Bounds::builder()
            .left(rect.left)
            .top(rect.top)
            .width(rect.w)
            .height(rect.h)
            .build();
        if let Err(e) = self
            .page
            .execute(SetWindowBoundsParams::new(window_id, bounds))
            .await
        {
            warn!("could not set window bounds: {e}");
        }
    }

    /// Bring the browser to the foreground. Full-display capture records
    /// whatever app is frontmost, so we both raise the tab within Chromium
    /// (`Page.bringToFront`) and activate the Chromium app at the macOS level
    /// via `open -a` (which, unlike `osascript activate`, needs no Automation
    /// permission and won't spawn a new window for an already-running app).
    pub async fn bring_to_front(&self) {
        if let Err(e) = self.page.bring_to_front().await {
            warn!("bring_to_front failed: {e}");
        }
        if let Some(app) = &self.app_bundle {
            let _ = tokio::process::Command::new("open")
                .arg("-a")
                .arg(app)
                .status()
                .await;
        }
    }

    /// Close the browser, stop the handler task, and remove the run's profile.
    pub async fn close(mut self) {
        let _ = self.browser.close().await;
        let _ = self.browser.wait().await;
        if let Some(h) = self.handler.take() {
            h.abort();
        }
        // Best-effort cleanup of the ephemeral profile.
        let _ = std::fs::remove_dir_all(&self.profile_dir);
    }
}
