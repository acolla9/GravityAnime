//! External player launching, modelled on `play_episode` in ani-cli 5.1.6.
//!
//! The provider's HLS streams only play with the embed site's `Referer`, and the
//! audio track / subtitles come as separate URLs, so a webview `<video>` tag can't
//! play them directly. Like ani-cli, we hand everything to mpv (default) or VLC.
//! Keep GPL-3.0-or-later notices when distributing derived work.

use anyhow::{anyhow, bail, Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKind {
    Mpv,
    Vlc,
}

impl PlayerKind {
    pub fn label(self) -> &'static str {
        match self {
            PlayerKind::Mpv => "mpv",
            PlayerKind::Vlc => "VLC",
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            PlayerKind::Mpv => "mpv",
            PlayerKind::Vlc => "vlc",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub path: PathBuf,
    pub kind: PlayerKind,
}

impl Player {
    /// Guess the kind from the file name (used for the env-var override).
    pub fn from_path(path: PathBuf) -> Self {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let kind = if stem.contains("vlc") {
            PlayerKind::Vlc
        } else {
            PlayerKind::Mpv
        };
        Self { path, kind }
    }

    pub fn display_name(&self) -> String {
        format!("{} ({})", self.kind.label(), self.path.display())
    }
}

/// Everything a player needs to open one episode.
pub struct PlayRequest<'a> {
    pub title: &'a str,
    pub video_url: &'a str,
    pub referer: &'a str,
    pub audio_url: Option<&'a str>,
    pub subtitle_url: Option<&'a str>,
    /// mpv only: Lua script that writes `eof_marker` when playback reaches the end of the file.
    pub eof_script: Option<&'a str>,
    pub eof_marker: Option<&'a str>,
    /// Seconds to start from (0 = the beginning).
    pub start_secs: u64,
}

/// mpv script: creates the marker file only when the episode played to its end
/// (reason "eof"), not when the user quits early. Lets the app tell "finished" from "closed".
pub const EOF_SCRIPT: &str = r#"local marker = mp.get_opt("anigui-eof-file")
local pos_file = marker and (marker .. ".pos")
local last_pos, last_dur = 0, 0

mp.observe_property("time-pos", "number", function(_, v)
    if v then last_pos = v end
end)
mp.observe_property("duration", "number", function(_, v)
    if v then last_dur = v end
end)

mp.register_event("end-file", function(event)
    if marker and event.reason == "eof" then
        local f = io.open(marker, "w")
        if f then
            f:write("eof")
            f:close()
        end
    end
end)

mp.register_event("shutdown", function()
    if pos_file then
        local f = io.open(pos_file, "w")
        if f then
            f:write(string.format("%d,%d", math.floor(last_pos), math.floor(last_dur)))
            f:close()
        end
    end
end)
"#;

/// Write the script into `dir` and return its path.
pub fn write_eof_script(dir: &Path) -> std::io::Result<PathBuf> {
    let path = dir.join("anigui-eof.lua");
    std::fs::write(&path, EOF_SCRIPT)?;
    Ok(path)
}

/// Build the argument list. Same flags ani-cli uses for mpv and VLC.
pub fn build_args(kind: PlayerKind, req: &PlayRequest) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    match kind {
        PlayerKind::Mpv => {
            args.push(format!("--referrer={}", req.referer).into());
            if let Some(audio) = req.audio_url {
                args.push(format!("--audio-file={audio}").into());
            }
            if let Some(sub) = req.subtitle_url {
                args.push(format!("--sub-file={sub}").into());
            }
            if let (Some(script), Some(marker)) = (req.eof_script, req.eof_marker) {
                args.push(format!("--script={script}").into());
                args.push(format!("--script-opts=anigui-eof-file={marker}").into());
            }
            if req.start_secs > 0 {
                args.push(format!("--start={}", req.start_secs).into());
            }
            args.push(format!("--force-media-title={}", req.title).into());
            args.push(req.video_url.into());
        }
        PlayerKind::Vlc => {
            args.push(format!("--http-referrer={}", req.referer).into());
            args.push("--play-and-exit".into());
            if req.start_secs > 0 {
                args.push(format!("--start-time={}", req.start_secs).into());
            }
            args.push(format!("--meta-title={}", req.title).into());
            args.push(req.video_url.into());
            let slaves: Vec<&str> = [req.audio_url, req.subtitle_url]
                .into_iter()
                .flatten()
                .collect();
            if !slaves.is_empty() {
                args.push(format!(":input-slave={}", slaves.join("#")).into());
            }
        }
    }
    args
}

/// Look up an executable on PATH (adds `.exe` on Windows).
fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let plain = dir.join(name);
        if plain.is_file() {
            return Some(plain);
        }
        if cfg!(windows) {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// Resolve a user-supplied setting: either a path to the executable or a command name.
fn resolve_override(value: &str) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    let p = Path::new(value);
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    find_on_path(value)
}

/// Well-known install locations for each player.
fn candidate_paths(kind: PlayerKind) -> Vec<PathBuf> {
    let mut out = Vec::new();
    match kind {
        PlayerKind::Mpv => {
            // A copy shipped next to the app.
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    for rel in [
                        "mpv.exe",
                        "mpv/mpv.exe",
                        "resources/mpv/mpv.exe",
                        "mpv",
                        "resources/mpv/mpv",
                    ] {
                        out.push(dir.join(rel));
                    }
                }
            }
            if cfg!(windows) {
                if let Some(home) = std::env::var_os("USERPROFILE") {
                    let home = PathBuf::from(home);
                    out.push(home.join("scoop/apps/mpv/current/mpv.exe"));
                    out.push(home.join("AppData/Local/Programs/mpv/mpv.exe"));
                    out.push(home.join("AppData/Local/Microsoft/WinGet/Links/mpv.exe"));
                }
                for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                    if let Some(pf) = std::env::var_os(var) {
                        out.push(PathBuf::from(pf).join("mpv/mpv.exe"));
                    }
                }
                if let Some(choco) = std::env::var_os("ChocolateyInstall") {
                    let choco = PathBuf::from(choco);
                    out.push(choco.join("bin/mpv.exe"));
                    out.push(choco.join("lib/mpvio.install/tools/mpv.exe"));
                }
                // Chocolatey's default location, in case ChocolateyInstall isn't set for this process.
                if let Some(data) = std::env::var_os("ProgramData") {
                    out.push(
                        PathBuf::from(data).join("chocolatey/lib/mpvio.install/tools/mpv.exe"),
                    );
                }
            }
        }
        PlayerKind::Vlc => {
            if cfg!(windows) {
                for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                    if let Some(pf) = std::env::var_os(var) {
                        out.push(PathBuf::from(pf).join("VideoLAN/VLC/vlc.exe"));
                    }
                }
            }
            out.push(PathBuf::from("/Applications/VLC.app/Contents/MacOS/VLC"));
        }
    }
    out
}

/// Find the requested player. Order: the custom path from Settings, the
/// `ANIGUI_PLAYER` / `ANI_CLI_PLAYER` environment variable (if it points at that
/// kind of player), well-known install locations, then PATH.
pub fn find_player(kind: PlayerKind, custom: Option<&str>) -> Option<Player> {
    if let Some(value) = custom.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(path) = resolve_override(value) {
            return Some(Player { path, kind });
        }
    }
    for var in ["ANIGUI_PLAYER", "ANI_CLI_PLAYER"] {
        if let Ok(value) = std::env::var(var) {
            if let Some(path) = resolve_override(value.trim()) {
                let player = Player::from_path(path);
                if player.kind == kind {
                    return Some(player);
                }
            }
        }
    }
    if let Some(path) = candidate_paths(kind).into_iter().find(|p| p.is_file()) {
        return Some(Player { path, kind });
    }
    find_on_path(kind.command_name()).map(|path| Player { path, kind })
}

pub fn missing_message(kind: PlayerKind) -> String {
    match kind {
        PlayerKind::Mpv => "mpv wasn't found. Install it (`choco install mpvio.install`, `scoop install mpv` or `winget install shinchiro.mpv`, or see https://mpv.io/installation/), set its location in Settings, or switch to VLC in Settings.".to_string(),
        PlayerKind::Vlc => "VLC wasn't found. Install it from https://www.videolan.org/vlc/, set its location in Settings, or switch to mpv in Settings.".to_string(),
    }
}

/// Start the player without blocking. The returned child should be waited on
/// (see `lib.rs`) so it is reaped and its exit code can be reported.
pub fn spawn(player: &Player, req: &PlayRequest) -> Result<Child> {
    if req.video_url.trim().is_empty() {
        bail!("No video URL to play");
    }
    let mut cmd = Command::new(&player.path);
    cmd.args(build_args(player.kind, req))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000; // no console flash for mpv.exe
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
        .with_context(|| format!("could not start {}", player.path.display()))
        .map_err(|e| anyhow!("{e:#}"))
}

/// mpv exit codes: 0 ok, 1 init error, 2 file couldn't be played, 3 some files
/// couldn't be played, 4 force-quit/signal. 1 and 2 mean nothing played.
/// VLC reports 0 on a normal exit.
pub fn exit_code_is_failure(kind: PlayerKind, code: Option<i32>) -> bool {
    match (kind, code) {
        (PlayerKind::Mpv, Some(1 | 2)) => true,
        (PlayerKind::Vlc, Some(c)) => c != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(audio: Option<&'a str>, sub: Option<&'a str>) -> PlayRequest<'a> {
        PlayRequest {
            title: "Cyberpunk: Edgerunners Episode 4",
            video_url: "https://cdn.example/720/index.m3u8",
            referer: "https://embed.example/",
            audio_url: audio,
            subtitle_url: sub,
            eof_script: None,
            eof_marker: None,
            start_secs: 0,
        }
    }

    fn strs(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn mpv_args_match_ani_cli() {
        let args = strs(build_args(
            PlayerKind::Mpv,
            &req(
                Some("https://cdn.example/a.m3u8"),
                Some("https://cdn.example/s.vtt"),
            ),
        ));
        assert_eq!(
            args,
            vec![
                "--referrer=https://embed.example/",
                "--audio-file=https://cdn.example/a.m3u8",
                "--sub-file=https://cdn.example/s.vtt",
                "--force-media-title=Cyberpunk: Edgerunners Episode 4",
                "https://cdn.example/720/index.m3u8",
            ]
        );
    }

    #[test]
    fn mpv_omits_missing_audio_and_subs() {
        let args = strs(build_args(PlayerKind::Mpv, &req(None, None)));
        assert_eq!(args.len(), 3);
        assert!(!args
            .iter()
            .any(|a| a.starts_with("--audio-file") || a.starts_with("--sub-file")));
        assert_eq!(args.last().unwrap(), "https://cdn.example/720/index.m3u8");
    }

    #[test]
    fn mpv_gets_eof_script_but_vlc_does_not() {
        let mut r = req(None, None);
        r.eof_script = Some("C:/Users/me/AppData/anigui-eof.lua");
        r.eof_marker = Some("C:/Users/me/AppData/play-1.eof");
        let mpv = strs(build_args(PlayerKind::Mpv, &r));
        assert!(mpv.contains(&"--script=C:/Users/me/AppData/anigui-eof.lua".to_string()));
        assert!(mpv
            .contains(&"--script-opts=anigui-eof-file=C:/Users/me/AppData/play-1.eof".to_string()));
        let vlc = strs(build_args(PlayerKind::Vlc, &r));
        assert!(!vlc
            .iter()
            .any(|a| a.contains("script") || a.contains("eof")));
    }

    #[test]
    fn vlc_args_join_slaves_with_hash() {
        let args = strs(build_args(PlayerKind::Vlc, &req(Some("A"), Some("S"))));
        assert_eq!(args.last().unwrap(), ":input-slave=A#S");
        assert!(args.contains(&"--http-referrer=https://embed.example/".to_string()));
        let only_sub = strs(build_args(PlayerKind::Vlc, &req(None, Some("S"))));
        assert_eq!(only_sub.last().unwrap(), ":input-slave=S");
        let none = strs(build_args(PlayerKind::Vlc, &req(None, None)));
        assert!(!none.iter().any(|a| a.starts_with(":input-slave")));
    }

    #[test]
    fn kind_is_detected_from_file_name() {
        assert_eq!(
            Player::from_path("C:/Program Files/VideoLAN/VLC/vlc.exe".into()).kind,
            PlayerKind::Vlc
        );
        assert_eq!(
            Player::from_path("/usr/bin/mpv".into()).kind,
            PlayerKind::Mpv
        );
    }

    #[test]
    fn failure_codes() {
        assert!(exit_code_is_failure(PlayerKind::Mpv, Some(2)));
        assert!(!exit_code_is_failure(PlayerKind::Mpv, Some(0)));
        assert!(!exit_code_is_failure(PlayerKind::Mpv, Some(4)));
        assert!(!exit_code_is_failure(PlayerKind::Mpv, None));
        assert!(exit_code_is_failure(PlayerKind::Vlc, Some(1)));
    }

    #[test]
    fn empty_url_is_rejected() {
        let player = Player {
            path: "/bin/true".into(),
            kind: PlayerKind::Mpv,
        };
        let mut r = req(None, None);
        r.video_url = " ";
        assert!(spawn(&player, &r).is_err());
    }

    #[test]
    fn custom_path_wins_and_keeps_the_chosen_kind() {
        let dir = std::env::temp_dir().join("anigui_find_player_test");
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("some-player.bin");
        std::fs::write(&fake, "x").unwrap();
        let p = find_player(PlayerKind::Vlc, Some(fake.to_str().unwrap())).unwrap();
        assert_eq!(p.kind, PlayerKind::Vlc);
        assert_eq!(p.path, fake);
        // A bad custom path falls through to auto-detection (which may or may not find one).
        let _ = find_player(PlayerKind::Mpv, Some("/definitely/not/here/mpv"));
    }

    #[test]
    fn eof_script_is_written() {
        let dir = std::env::temp_dir().join("anigui_eof_script_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_eof_script(&dir).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(
            text.contains("end-file")
                && text.contains("\"eof\"")
                && text.contains("anigui-eof-file")
        );
    }

    #[test]
    fn missing_messages_point_to_the_other_player() {
        assert!(missing_message(PlayerKind::Mpv).contains("switch to VLC"));
        assert!(missing_message(PlayerKind::Vlc).contains("switch to mpv"));
    }
}
