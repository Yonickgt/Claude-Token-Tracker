//! Self-update via GitHub Releases: find a newer tag, download its `*-Setup.exe`, run it silently.
use serde_json::Value;
use std::{io::Write, time::Duration};

const LATEST: &str = "https://api.github.com/repos/Yonickgt/Claude-Token-Tracker/releases/latest";

#[derive(Clone, Debug)]
pub struct Update { pub tag: String, pub url: String }

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).user_agent("claude-token-widget").build().new_agent()
}

/// "v1.2.3" -> (1, 2, 3); anything unparseable is (0, 0, 0) so it never counts as newer.
fn ver(s: &str) -> (u32, u32, u32) {
    let mut n = s.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0));
    (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0))
}

/// `Some` only when the latest release is newer than this build and ships a Setup.exe.
pub fn check() -> Option<Update> {
    let body: Value = agent().get(LATEST).header("Accept", "application/vnd.github+json").call().ok()?.body_mut().read_json().ok()?;
    let tag = body["tag_name"].as_str()?;
    if ver(tag) <= ver(env!("CARGO_PKG_VERSION")) { return None }
    let url = body["assets"].as_array()?.iter()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with("-Setup.exe")))?["browser_download_url"].as_str()?;
    Some(Update { tag: tag.into(), url: url.into() })
}

/// Downloads the installer and launches it; the caller then exits so the installer can replace the exe.
pub fn install(u: &Update) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("ClaudeTokenWidget-{}-Setup.exe", u.tag));
    let mut resp = agent().get(&u.url).call().map_err(|e| e.to_string())?;
    let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    std::io::copy(&mut resp.body_mut().as_reader(), &mut f).map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    std::process::Command::new(&path).args(["/SILENT", "/CLOSEAPPLICATIONS", "/RESTARTAPPLICATIONS"]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ver;
    #[test]
    fn compares_versions() {
        assert!(ver("v0.2.0") > ver("0.1.9"));
        assert!(ver("v1.10.0") > ver("v1.9.0"));
        assert!(ver("junk") == (0, 0, 0));
    }
}
