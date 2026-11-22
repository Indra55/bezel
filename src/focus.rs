use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tracing::debug;

use crate::config::Config;

const IPC_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, PartialEq, Eq)]
pub struct FocusedWindow {
    pub app_id: String,
    pub id: String,
}

pub type Focus = Option<Arc<FocusedWindow>>;

pub fn same_focus(left: &Focus, right: &Focus) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn publish(tx: &watch::Sender<Focus>, window: Option<FocusedWindow>) {
    tx.send_if_modified(|previous| {
        if previous.as_deref() == window.as_ref() {
            return false;
        }
        // Returning to a window creates a new snapshot and invalidates older gestures.
        *previous = window.map(Arc::new);
        true
    });
}

#[derive(Clone)]
pub enum Compositor {
    Niri(PathBuf),
    Hyprland(PathBuf),
}

impl Compositor {
    pub fn detect() -> Option<Self> {
        if let Some(path) = std::env::var_os("NIRI_SOCKET").filter(|path| !path.is_empty()) {
            return Some(Self::Niri(path.into()));
        }
        let runtime = dirs::runtime_dir()?;
        let instance = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        if instance.is_empty() {
            return None;
        }
        Some(Self::Hyprland(runtime.join("hypr").join(instance)))
    }

    pub async fn monitor(self, mut config: watch::Receiver<Config>, tx: watch::Sender<Focus>) {
        while config.wait_for(|cfg| !cfg.apps.is_empty()).await.is_ok() {
            let result = tokio::select! {
                result = self.read_events(&tx) => Some(result),
                stopped = config.wait_for(|cfg| cfg.apps.is_empty()) => {
                    if stopped.is_err() {
                        return;
                    }
                    None
                }
            };
            publish(&tx, None);
            if let Some(Err(err)) = result {
                debug!(%err, "Waiting to reconnect to compositor focus events");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    pub async fn focused_window(&self) -> Result<Option<FocusedWindow>> {
        timeout(IPC_TIMEOUT, async {
            match self {
                Self::Niri(path) => {
                    let (mut reply, _) = niri_request(path, b"\"FocusedWindow\"\n").await?;
                    let window = reply
                        .get_mut("FocusedWindow")
                        .context("Missing FocusedWindow in niri response")?;
                    let window: Option<NiriWindow> = serde_json::from_value(window.take())?;
                    Ok(window.as_ref().and_then(NiriWindow::focus))
                }
                Self::Hyprland(instance) => {
                    let mut stream = UnixStream::connect(instance.join(".socket.sock")).await?;
                    stream.write_all(b"j/activewindow").await?;
                    let mut reply = Vec::new();
                    stream.read_to_end(&mut reply).await?;
                    hypr_window(&reply)
                }
            }
        })
        .await
        .context("Timed out querying the focused window")?
    }

    pub async fn verify_focus(
        &self,
        focus: &watch::Receiver<Focus>,
        target: &Arc<FocusedWindow>,
    ) -> Result<bool> {
        let unchanged = || {
            focus
                .borrow()
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, target))
        };
        if !unchanged() {
            return Ok(false);
        }
        let current = self.focused_window().await?;
        Ok(current.as_ref() == Some(target.as_ref()) && unchanged())
    }

    async fn read_events(&self, tx: &watch::Sender<Focus>) -> Result<()> {
        match self {
            Self::Niri(path) => {
                let (reply, mut lines) =
                    timeout(IPC_TIMEOUT, niri_request(path, b"\"EventStream\"\n"))
                        .await
                        .context("Timed out subscribing to niri events")??;
                ensure!(reply == "Handled", "Unexpected reply to niri EventStream");
                let mut state = NiriState::default();
                while let Some(line) = lines.next_line().await? {
                    if state.update(serde_json::from_str(&line)?)? {
                        publish(tx, state.window());
                    }
                }
            }
            Self::Hyprland(instance) => {
                let stream = UnixStream::connect(instance.join(".socket2.sock")).await?;
                let mut lines = BufReader::new(stream).lines();
                publish(tx, self.focused_window().await?);
                while let Some(line) = lines.next_line().await? {
                    let Some(address) = line.strip_prefix("activewindowv2>>") else {
                        continue;
                    };
                    let address = hypr_address(address);
                    if tx.borrow().as_ref().map(|w| w.id.as_str()) != address.as_deref() {
                        publish(tx, None);
                    }
                    if let Some(address) = address {
                        let window = self.focused_window().await?.filter(|w| w.id == address);
                        publish(tx, window);
                    }
                }
            }
        }
        bail!("Compositor event socket disconnected")
    }
}

async fn niri_request(
    path: &Path,
    request: &[u8],
) -> Result<(Value, Lines<BufReader<UnixStream>>)> {
    let mut stream = UnixStream::connect(path).await?;
    stream.write_all(request).await?;
    let mut lines = BufReader::new(stream).lines();
    let line = lines.next_line().await?.context("Empty niri reply")?;
    let mut reply: Value = serde_json::from_str(&line)?;
    let response = reply
        .get_mut("Ok")
        .map(Value::take)
        .with_context(|| format!("niri rejected the IPC request: {reply}"))?;
    Ok((response, lines))
}

#[derive(Deserialize)]
struct NiriWindow {
    id: u64,
    app_id: Option<String>,
    is_focused: bool,
}

impl NiriWindow {
    fn focus(&self) -> Option<FocusedWindow> {
        Some(FocusedWindow {
            app_id: self.app_id.as_ref().filter(|id| !id.is_empty())?.clone(),
            id: self.id.to_string(),
        })
    }
}

#[derive(Default)]
struct NiriState {
    windows: HashMap<u64, NiriWindow>,
    focused: Option<u64>,
}

impl NiriState {
    fn update(&mut self, mut event: Value) -> Result<bool> {
        if let Some(windows) = event.pointer_mut("/WindowsChanged/windows") {
            let windows: Vec<NiriWindow> = serde_json::from_value(windows.take())?;
            self.focused = windows.iter().find(|w| w.is_focused).map(|w| w.id);
            self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
        } else if let Some(window) = event.pointer_mut("/WindowOpenedOrChanged/window") {
            let window: NiriWindow = serde_json::from_value(window.take())?;
            if window.is_focused {
                self.focused = Some(window.id);
            } else if self.focused == Some(window.id) {
                self.focused = None;
            }
            self.windows.insert(window.id, window);
        } else if let Some(id) = event.pointer_mut("/WindowFocusChanged/id") {
            self.focused = serde_json::from_value(id.take())?;
        } else if let Some(id) = event.pointer("/WindowClosed/id").and_then(Value::as_u64) {
            self.windows.remove(&id);
            if self.focused == Some(id) {
                self.focused = None;
            }
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn window(&self) -> Option<FocusedWindow> {
        self.windows.get(&self.focused?)?.focus()
    }
}

fn hypr_address(address: &str) -> Option<String> {
    let address = address.trim();
    let raw = address.strip_prefix("0x").unwrap_or(address);
    let raw = raw.trim_start_matches('0');
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", raw.to_ascii_lowercase()))
}

fn hypr_window(reply: &[u8]) -> Result<Option<FocusedWindow>> {
    let reply: Value = serde_json::from_slice(reply)?;
    let Some(app_id) = reply
        .get("class")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    Ok(reply
        .get("address")
        .and_then(Value::as_str)
        .and_then(hypr_address)
        .map(|id| FocusedWindow {
            app_id: app_id.to_owned(),
            id,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::net::UnixListener;

    struct SocketDir(PathBuf);

    impl SocketDir {
        fn new() -> Self {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("bezel-ipc-{}-{now}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for SocketDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn expect_focus(rx: &mut watch::Receiver<Focus>, id: Option<&str>) -> Focus {
        timeout(
            Duration::from_secs(2),
            rx.wait_for(|focus| focus.as_ref().map(|w| w.id.as_str()) == id),
        )
        .await
        .unwrap()
        .unwrap()
        .clone()
    }

    async fn respond(socket: &UnixListener, request: &[u8], reply: &[u8]) {
        let (mut client, _) = socket.accept().await.unwrap();
        let mut received = vec![0; request.len()];
        client.read_exact(&mut received).await.unwrap();
        assert_eq!(received, request);
        client.write_all(reply).await.unwrap();
    }

    #[tokio::test]
    async fn niri_snapshot_cannot_dispatch_after_focus_moves_or_returns() {
        timeout(Duration::from_secs(5), async {
            let dir = SocketDir::new();
            let path = dir.0.join("niri.sock");
            let socket = UnixListener::bind(&path).unwrap();
            let compositor = Compositor::Niri(path);
            let (tx, mut focus) = watch::channel(None);
            let monitor = tokio::spawn({
                let compositor = compositor.clone();
                async move { compositor.read_events(&tx).await }
            });
            let (mut events, _) = socket.accept().await.unwrap();
            let mut request = [0; b"\"EventStream\"\n".len()];
            events.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"\"EventStream\"\n");
            events.write_all(concat!(
                "{\"Ok\":\"Handled\"}\n",
                "{\"WindowsChanged\":{\"windows\":[{\"id\":1,\"app_id\":\"firefox\",\"is_focused\":true},",
                "{\"id\":2,\"app_id\":\"firefox\",\"is_focused\":false}]}}\n"
            ).as_bytes()).await.unwrap();
            let captured = expect_focus(&mut focus, Some("1")).await.unwrap();

            for id in [1, 2] {
                let reply = format!("{{\"Ok\":{{\"FocusedWindow\":{{\"id\":{id},\"app_id\":\"firefox\",\"is_focused\":true}}}}}}\n");
                let (verified, ()) = tokio::join!(
                    compositor.verify_focus(&focus, &captured),
                    respond(&socket, b"\"FocusedWindow\"\n", reply.as_bytes()),
                );
                assert_eq!(verified.unwrap(), id == 1);
            }
            events.write_all(b"{\"WindowFocusChanged\":{\"id\":2}}\n").await.unwrap();
            expect_focus(&mut focus, Some("2")).await;
            assert!(!compositor.verify_focus(&focus, &captured).await.unwrap());
            events.write_all(b"{\"WindowFocusChanged\":{\"id\":1}}\n").await.unwrap();
            expect_focus(&mut focus, Some("1")).await;
            assert!(!compositor.verify_focus(&focus, &captured).await.unwrap());
            monitor.abort();
        }).await.unwrap();
    }

    #[tokio::test]
    async fn hyprland_tracks_window_addresses_and_clears_focus() {
        timeout(Duration::from_secs(5), async {
            let dir = SocketDir::new();
            let commands = UnixListener::bind(dir.0.join(".socket.sock")).unwrap();
            let event_socket = UnixListener::bind(dir.0.join(".socket2.sock")).unwrap();
            let (tx, mut focus) = watch::channel(None);
            let compositor = Compositor::Hyprland(dir.0.clone());
            let monitor = tokio::spawn({
                let compositor = compositor.clone();
                async move { compositor.read_events(&tx).await }
            });
            let (mut events, _) = event_socket.accept().await.unwrap();
            respond(
                &commands,
                b"j/activewindow",
                br#"{"address":"0xa11","class":"Code"}"#,
            )
            .await;
            let captured = expect_focus(&mut focus, Some("0xa11")).await.unwrap();
            assert_eq!(captured.app_id, "Code");

            events.write_all(b"activewindowv2>>0B22\n").await.unwrap();
            respond(
                &commands,
                b"j/activewindow",
                br#"{"address":"0xb22","class":"Code"}"#,
            )
            .await;
            expect_focus(&mut focus, Some("0xb22")).await;
            assert!(!compositor.verify_focus(&focus, &captured).await.unwrap());
            events.write_all(b"activewindowv2>>\n").await.unwrap();
            expect_focus(&mut focus, None).await;
            monitor.abort();
        })
        .await
        .unwrap();
    }
}
