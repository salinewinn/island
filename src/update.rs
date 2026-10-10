//! Auto-update from GitHub Releases.
//!
//! The game asks the GitHub API for the latest release of a public repository. If its tag is newer
//! than this build, the player can download `Island.exe` from that release (checked against an
//! optional `Island.exe.sha256` asset) and restart into the new version. HTTPS is handled by the
//! `curl.exe` that ships with Windows 10/11, so no extra crates are needed.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Version of this build (from Cargo.toml). Release tags look like `v0.5.1`.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Repository to check unless the settings file overrides it (`update_repo=owner/name`).
pub const DEFAULT_REPO: &str = "salinewinn/island";
/// The release asset that is the game itself.
const ASSET: &str = "Island.exe";

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available { tag: String, url: String, sha_url: Option<String>, notes: String },
    Downloading { tag: String },
    Ready { tag: String },
    Failed(String),
    /// no repository configured yet
    Disabled,
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

pub fn state() -> State {
    STATE.lock().map(|s| s.clone()).unwrap_or(State::Idle)
}

fn set(s: State) {
    if let Ok(mut g) = STATE.lock() {
        *g = s;
    }
}

/// Lets the UI show an error from the final install step.
pub fn report(msg: &str) {
    set(State::Failed(msg.to_string()));
}

/// Debug only (--fakeupdate): pretend a newer release exists so the UI can be checked.
pub fn debug_fake_available() {
    set(State::Available { tag: "v0.5.1".into(), url: String::new(), sha_url: None, notes: String::new() });
}

pub fn repo_configured(repo: &str) -> bool {
    let r = repo.trim();
    r.contains('/') && !r.contains("YOUR_") && r.len() > 3
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------
fn curl_cmd() -> Command {
    let mut c = Command::new("curl.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c.args(["-s", "-S", "-L", "--fail", "-H", "User-Agent: island-game-updater"]);
    if let Ok(tok) = std::env::var("ISLAND_GITHUB_TOKEN") {
        if !tok.trim().is_empty() {
            c.args(["-H", &format!("Authorization: Bearer {}", tok.trim())]);
        }
    }
    c
}

fn http_get(url: &str, seconds: u32) -> Result<String, String> {
    let out = curl_cmd()
        .args(["--max-time", &seconds.to_string(), "-H", "Accept: application/vnd.github+json", url])
        .output()
        .map_err(|e| format!("curl.exe isn't available ({e})"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "couldn't reach GitHub".to_string() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn http_download(url: &str, dest: &PathBuf) -> Result<(), String> {
    let out = curl_cmd()
        .args(["--max-time", "900", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| format!("curl.exe isn't available ({e})"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(dest);
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "the download failed".to_string() } else { err });
    }
    Ok(())
}

/// First JSON string value after `"key":` (handles the common escapes).
fn json_str(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = json.find(&pat)? + pat.len();
    let rest = &json[i..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut esc = false;
    for c in chars {
        if esc {
            out.push(match c {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

/// Every `"browser_download_url": "..."` value in the release JSON.
fn asset_urls(json: &str) -> Vec<String> {
    let mut v = vec![];
    let key = "\"browser_download_url\"";
    let mut from = 0;
    while let Some(i) = json[from..].find(key) {
        let at = from + i;
        if let Some(u) = json_str(&json[at..], "browser_download_url") {
            v.push(u);
        }
        from = at + key.len();
    }
    v
}

pub struct Release {
    pub tag: String,
    pub url: Option<String>,
    pub sha_url: Option<String>,
    pub notes: String,
}

pub fn parse_release(json: &str) -> Option<Release> {
    let tag = json_str(json, "tag_name")?;
    let urls = asset_urls(json);
    let url = urls.iter().find(|u| u.ends_with(&format!("/{ASSET}"))).cloned();
    let sha_url = urls.iter().find(|u| u.ends_with(&format!("/{ASSET}.sha256"))).cloned();
    let notes = json_str(json, "body").unwrap_or_default();
    Some(Release { tag, url, sha_url, notes })
}

fn parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
        .take_while(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Is `remote` a newer version than `local`?
pub fn newer(remote: &str, local: &str) -> bool {
    let (a, b) = (parts(remote), parts(local));
    let n = a.len().max(b.len());
    for i in 0..n {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

fn exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

fn sibling(suffix: &str) -> Result<PathBuf, String> {
    let exe = exe_path()?;
    let mut name = exe.file_name().ok_or("no file name")?.to_os_string();
    name.push(suffix);
    Ok(exe.with_file_name(name))
}

fn sha256_of(path: &PathBuf) -> Result<String, String> {
    let mut c = Command::new("certutil");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.args(["-hashfile"]).arg(path).arg("SHA256").output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    text.lines()
        .map(|l| l.replace(' ', "").to_lowercase())
        .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| "couldn't compute the checksum".to_string())
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Removes the previous version left behind by the last update. Call once at start-up.
pub fn cleanup_old() {
    if let Ok(p) = sibling(".old") {
        let _ = std::fs::remove_file(p);
    }
    if let Ok(p) = sibling(".new") {
        let _ = std::fs::remove_file(p);
    }
}

/// Starts a background check for a newer release. Does nothing while a check/download is running.
pub fn check(repo: &str) {
    if !repo_configured(repo) {
        set(State::Disabled);
        return;
    }
    match state() {
        State::Checking | State::Downloading { .. } | State::Ready { .. } => return,
        _ => {}
    }
    set(State::Checking);
    let repo = repo.trim().to_string();
    std::thread::spawn(move || {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        match http_get(&url, 12) {
            Err(e) => set(State::Failed(format!("Couldn't check for updates: {e}"))),
            Ok(json) => match parse_release(&json) {
                None => set(State::Failed("GitHub sent a reply the game didn't understand".into())),
                Some(r) => {
                    if !newer(&r.tag, CURRENT) {
                        set(State::UpToDate);
                    } else if let Some(url) = r.url {
                        set(State::Available { tag: r.tag, url, sha_url: r.sha_url, notes: r.notes });
                    } else {
                        set(State::Failed(format!("Release {} has no {ASSET} file attached", r.tag)));
                    }
                }
            },
        }
    });
}

static AUTO_INSTALL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One click: download the update and install + restart as soon as it has finished.
pub fn download_and_install() {
    AUTO_INSTALL.store(true, std::sync::atomic::Ordering::SeqCst);
    download();
}

/// True once (after a one-click update has finished downloading): the caller should now call
/// `apply_and_restart`.
pub fn take_auto_install() -> bool {
    matches!(state(), State::Ready { .. }) && AUTO_INSTALL.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// Downloads the update that `check` found (a no-op unless one is available).
pub fn download() {
    let State::Available { tag, url, sha_url, .. } = state() else { return };
    set(State::Downloading { tag: tag.clone() });
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let dest = sibling(".new")?;
            http_download(&url, &dest)?;
            let meta = std::fs::metadata(&dest).map_err(|e| e.to_string())?;
            let head = std::fs::read(&dest).map(|b| b.iter().take(2).copied().collect::<Vec<u8>>()).unwrap_or_default();
            if meta.len() < 300_000 || head != b"MZ" {
                let _ = std::fs::remove_file(&dest);
                return Err("the downloaded file doesn't look like the game".into());
            }
            if let Some(su) = sha_url {
                let text = http_get(&su, 30)?;
                let want = text.split_whitespace().next().unwrap_or("").to_lowercase();
                let got = sha256_of(&dest)?;
                if want.len() == 64 && want != got {
                    let _ = std::fs::remove_file(&dest);
                    return Err("the download failed its safety check (checksum mismatch)".into());
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => set(State::Ready { tag }),
            Err(e) => {
                AUTO_INSTALL.store(false, std::sync::atomic::Ordering::SeqCst);
                set(State::Failed(format!("Update failed: {e}")));
            }
        }
    });
}

/// Swaps the downloaded file in and starts the new version. On success this never returns.
pub fn apply_and_restart() -> Result<(), String> {
    let State::Ready { .. } = state() else { return Err("no update is ready".into()) };
    let exe = exe_path()?;
    let new = sibling(".new")?;
    let old = sibling(".old")?;
    if !new.exists() {
        return Err("the downloaded file is missing".into());
    }
    let _ = std::fs::remove_file(&old);
    // a running exe can be renamed on Windows, just not overwritten
    std::fs::rename(&exe, &old).map_err(|e| format!("couldn't move the old version aside: {e}"))?;
    if let Err(e) = std::fs::rename(&new, &exe) {
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("couldn't install the new version: {e}"));
    }
    match Command::new(&exe).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            let _ = std::fs::remove_file(&exe);
            let _ = std::fs::rename(&old, &exe);
            Err(format!("couldn't start the new version: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"url":"https://api.github.com/repos/o/r/releases/1","tag_name":"v0.6.1","name":"Island 0.6.1","body":"New hermit trades\r\n- fixes","assets":[{"name":"Island.exe.sha256","browser_download_url":"https://github.com/o/r/releases/download/v0.6.1/Island.exe.sha256"},{"name":"Island.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.6.1/Island.exe"}]}"#;

    #[test]
    fn parses_release() {
        let r = parse_release(SAMPLE).unwrap();
        assert_eq!(r.tag, "v0.6.1");
        assert_eq!(r.url.unwrap(), "https://github.com/o/r/releases/download/v0.6.1/Island.exe");
        assert_eq!(r.sha_url.unwrap(), "https://github.com/o/r/releases/download/v0.6.1/Island.exe.sha256");
        assert!(r.notes.starts_with("New hermit trades\r\n"));
    }

    #[test]
    fn version_order() {
        assert!(newer("v0.6.0", "0.5.9"));
        assert!(newer("v1.0.0", "0.99.99"));
        assert!(newer("0.5.1", "0.5.0"));
        assert!(!newer("v0.5.0", "0.5.0"));
        assert!(!newer("0.4.9", "0.5.0"));
        assert!(newer("v0.5.0.1", "0.5.0"));
        assert!(!newer("garbage", "0.5.0"));
    }

    #[test]
    fn repo_check() {
        assert!(!repo_configured(DEFAULT_REPO));
        assert!(!repo_configured(""));
        assert!(repo_configured("someone/island"));
    }
}
