//! Background jobs that call free, open-source helper programs:
//! yt-dlp (YouTube audio), Demucs (stem splitting), Basic Pitch
//! (audio-to-notes) and Ollama (local open-source LLMs).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, PartialEq, Debug)]
pub enum JobState {
    Idle,
    Running,
    Done,
    Failed,
}

#[derive(Clone)]
pub struct Job {
    pub inner: Arc<Mutex<JobInner>>,
}

pub struct JobInner {
    pub state: JobState,
    pub log: Vec<String>,
    pub result: Option<String>,
    pub outputs: Vec<(String, PathBuf)>,
    /// Decoded stems (name, mono samples at the engine rate), ready for the timeline.
    pub stems: Vec<(String, Vec<f32>)>,
}

impl Default for Job {
    fn default() -> Self {
        Self { inner: Arc::new(Mutex::new(JobInner { state: JobState::Idle, log: vec![], result: None, outputs: vec![], stems: vec![] })) }
    }
}

impl Job {
    pub fn state(&self) -> JobState {
        self.inner.lock().map(|i| i.state.clone()).unwrap_or(JobState::Failed)
    }
    pub fn log(&self, s: impl Into<String>) {
        if let Ok(mut i) = self.inner.lock() {
            i.log.push(s.into());
            if i.log.len() > 400 {
                i.log.remove(0);
            }
        }
    }
    fn set(&self, st: JobState) {
        if let Ok(mut i) = self.inner.lock() {
            i.state = st;
        }
    }
    pub fn reset(&self) {
        if let Ok(mut i) = self.inner.lock() {
            i.state = JobState::Running;
            i.log.clear();
            i.result = None;
            i.outputs.clear();
            i.stems.clear();
        }
    }
    pub fn finish(&self, ok: bool) {
        self.set(if ok { JobState::Done } else { JobState::Failed });
    }
}

/// Folders where the helper programs live. An app opened from Finder gets only
/// /usr/bin:/bin, so Homebrew and Python user folders are added by hand.
fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("PATH").unwrap_or_default().split(':').filter(|s| !s.is_empty()).map(PathBuf::from).collect();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"].iter().map(PathBuf::from));
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(Path::new(&home).join(".local/bin"));
        if let Ok(rd) = std::fs::read_dir(Path::new(&home).join("Library/Python")) {
            let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin")).filter(|p| p.is_dir()).collect();
            v.sort();
            v.reverse();
            dirs.extend(v);
        }
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// PATH given to every helper, so e.g. yt-dlp and Demucs can find ffmpeg.
pub fn helper_path() -> String {
    tool_dirs().iter().map(|d| d.to_string_lossy().to_string()).collect::<Vec<_>>().join(":")
}

/// Find a program on PATH or in the usual Homebrew / Python user folders.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    tool_dirs().into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// A command for a helper program with the full helper PATH.
fn helper(prog: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(prog);
    c.env("PATH", helper_path());
    c
}

pub fn workspace_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let p = Path::new(&home).join("Music").join("Bandtools");
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Run a command, streaming its output lines into the job log.
fn run_logged(job: &Job, cmd: &mut Command) -> bool {
    job.log(format!("$ {:?}", cmd).replace('"', ""));
    let child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            job.log(format!("Could not start: {e}"));
            return false;
        }
    };
    let err = child.stderr.take();
    let j2 = job.clone();
    let t = std::thread::spawn(move || {
        if let Some(e) = err {
            for l in BufReader::new(e).split(b'\r').flatten() {
                let s = String::from_utf8_lossy(&l).trim().to_string();
                if !s.is_empty() {
                    j2.log(s.lines().last().unwrap_or("").to_string());
                }
            }
        }
    });
    if let Some(o) = child.stdout.take() {
        for l in BufReader::new(o).lines().map_while(Result::ok) {
            if !l.trim().is_empty() {
                job.log(l);
            }
        }
    }
    let _ = t.join();
    child.wait().map(|s| s.success()).unwrap_or(false)
}

pub const STEM_NAMES: [&str; 6] = ["guitar", "vocals", "drums", "bass", "piano", "other"];

/// Download (optional) and split a song into stems with Demucs' six-stem model.
pub fn split_stems(job: Job, source: String, model: String, sr: f32) {
    job.reset();
    std::thread::spawn(move || {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let dir = workspace_dir().join(format!("song-{stamp}"));
        let _ = std::fs::create_dir_all(&dir);
        let source = source.trim().trim_matches('"').trim_matches('\'').to_string();
        let mut audio = PathBuf::from(&source);
        if source.starts_with("http") {
            let Some(ytdlp) = find_tool("yt-dlp") else {
                job.log("yt-dlp is not installed. Use 'Install / update helpers' first.");
                job.finish(false);
                return;
            };
            job.log("Step 1/3: downloading the audio from YouTube…");
            let mut cmd = helper(&ytdlp);
            // --ignore-config: a personal yt-dlp config (subtitles, embedding…) must not change what we get.
            cmd.args(["--ignore-config", "--no-playlist", "-f", "bestaudio/best", "-x", "--audio-format", "wav", "--newline"]);
            if let Some(ff) = find_tool("ffmpeg") {
                cmd.arg("--ffmpeg-location").arg(ff);
            }
            cmd.arg("-o").arg(dir.join("source.%(ext)s")).arg(&source);
            let mut ok = run_logged(&job, &mut cmd);
            audio = dir.join("source.wav");
            if !ok || !audio.exists() {
                // An old yt-dlp is the usual cause (YouTube answers 403): update once and retry.
                if let Some(brew) = find_tool("brew") {
                    job.log("Download failed; updating yt-dlp and trying again…");
                    run_logged(&job, helper(brew).args(["upgrade", "yt-dlp"]));
                    ok = run_logged(&job, &mut cmd);
                }
            }
            if !ok || !audio.exists() {
                job.log("Download failed. Check the link and your internet connection, then press 'Install / update helpers'.");
                job.finish(false);
                return;
            }
        } else if !audio.exists() {
            job.log(format!("The file '{source}' does not exist."));
            job.finish(false);
            return;
        }
        job.log(format!("Step 2/3: splitting into stems with Demucs ({model})… this can take a few minutes."));
        let out = dir.join("separated");
        let ok = if let Some(d) = find_tool("demucs") {
            run_logged(&job, helper(d).args(["-n", &model, "-o"]).arg(&out).arg(&audio))
        } else {
            run_logged(&job, helper("python3").args(["-m", "demucs", "-n", &model, "-o"]).arg(&out).arg(&audio))
        };
        let stem = audio.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let folder = out.join(&model).join(&stem);
        let mut found = vec![];
        for n in STEM_NAMES {
            let p = folder.join(format!("{n}.wav"));
            if p.exists() {
                found.push((n.to_string(), p));
            }
        }
        if !ok || found.is_empty() {
            job.log("Splitting failed. Demucs may not be installed: use 'Install / update helpers'.");
            job.finish(false);
            return;
        }
        job.log(format!("Step 3/3: loading {} stems into the timeline…", found.len()));
        let mut stems = vec![];
        for (n, p) in &found {
            match crate::audio::load_wav(&p.to_string_lossy(), sr) {
                Ok(s) => stems.push((n.clone(), s)),
                Err(e) => job.log(format!("Could not read {n}.wav: {e}")),
            }
        }
        job.log(format!("Done: {} stems in {}", stems.len(), folder.display()));
        let ok = !stems.is_empty();
        if let Ok(mut i) = job.inner.lock() {
            i.outputs = found;
            i.stems = stems;
        }
        job.finish(ok);
    });
}

/// Install or update the free helper tools with Homebrew / pip (only when asked).
pub fn install_helpers(job: Job) {
    job.reset();
    std::thread::spawn(move || {
        let mut ok = true;
        if let Some(brew) = find_tool("brew") {
            let missing: Vec<&str> = ["yt-dlp", "ffmpeg", "ollama"].into_iter().filter(|t| find_tool(t).is_none()).collect();
            if !missing.is_empty() {
                ok &= run_logged(&job, helper(&brew).arg("install").args(&missing));
            }
            // YouTube changes often; an old yt-dlp gets "HTTP Error 403".
            run_logged(&job, helper(&brew).args(["upgrade", "yt-dlp"]));
        } else {
            job.log("Homebrew not found: install yt-dlp, ffmpeg and Ollama from their websites.");
        }
        if find_tool("demucs").is_none() || find_tool("basic-pitch").is_none() {
            ok &= run_logged(&job, helper("python3").args(["-m", "pip", "install", "--user", "--upgrade", "demucs", "basic-pitch"]));
        }
        job.log(if ok { "Helpers ready." } else { "Some installs failed; see above." });
        job.finish(ok);
    });
}

pub struct ToolStatus {
    pub ytdlp: bool,
    pub ffmpeg: bool,
    pub demucs: bool,
    pub basic_pitch: bool,
    pub ollama: bool,
}

pub fn tool_status() -> ToolStatus {
    let demucs = find_tool("demucs").is_some();
    ToolStatus {
        ytdlp: find_tool("yt-dlp").is_some(),
        ffmpeg: find_tool("ffmpeg").is_some(),
        demucs,
        basic_pitch: find_tool("basic-pitch").is_some(),
        ollama: find_tool("ollama").is_some(),
    }
}

// ------------------------------------------------------------- Ollama (local LLM)

pub const OLLAMA_URL: &str = "http://127.0.0.1:11434";

fn ollama_up() -> bool {
    Command::new("curl").args(["-s", "-m", "2", &format!("{OLLAMA_URL}/api/version")]).output().map(|o| o.status.success() && !o.stdout.is_empty()).unwrap_or(false)
}

/// Make sure the local AI server runs: start `ollama serve` in the background when
/// it isn't (it keeps running for later questions). Installs Ollama if missing.
pub fn ensure_ollama(job: &Job) -> Result<(), String> {
    if ollama_up() {
        return Ok(());
    }
    if find_tool("ollama").is_none() {
        let brew = find_tool("brew").ok_or("Ollama is not installed and Homebrew was not found. Install Ollama from ollama.com.")?;
        job.log("Installing Ollama (free local AI runner)…");
        run_logged(job, helper(brew).args(["install", "ollama"]));
    }
    let ollama = find_tool("ollama").ok_or("Ollama could not be installed.")?;
    job.log("Starting the local AI (Ollama)…");
    helper(ollama).arg("serve").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not start Ollama: {e}"))?;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        if ollama_up() {
            return Ok(());
        }
    }
    Err("Ollama did not start.".into())
}

/// Download a model into Ollama when it isn't there yet (progress in the job log).
pub fn ensure_model(job: &Job, model: &str) -> Result<(), String> {
    if ollama_models().iter().any(|m| m == model || m.trim_end_matches(":latest") == model) {
        return Ok(());
    }
    let ollama = find_tool("ollama").ok_or("Ollama is not installed.")?;
    job.log(format!("Downloading the AI model {model} (one time, a few GB)…"));
    if run_logged(job, helper(ollama).args(["pull", model])) {
        Ok(())
    } else {
        Err(format!("Could not download {model}."))
    }
}

/// Ask a local open-source LLM (through Ollama) for a JSON answer.
pub fn ollama_json(model: &str, prompt: &str) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({ "model": model, "prompt": prompt, "stream": false, "format": "json", "options": { "temperature": 0.3 } });
    let out = Command::new("curl")
        .args(["-s", "-m", "300", "-X", "POST", &format!("{OLLAMA_URL}/api/generate"), "-H", "Content-Type: application/json", "-d"])
        .arg(body.to_string())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err("The local AI (Ollama) did not answer.".into());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(format!("Ollama: {e}"));
    }
    let text = v.get("response").and_then(|r| r.as_str()).unwrap_or("{}");
    serde_json::from_str(text).map_err(|e| format!("The AI answer was not valid JSON: {e}"))
}

/// Names of models already downloaded in Ollama.
pub fn ollama_models() -> Vec<String> {
    let Ok(out) = Command::new("curl").args(["-s", "-m", "3", &format!("{OLLAMA_URL}/api/tags")]).output() else {
        return vec![];
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return vec![];
    };
    v.get("models")
        .and_then(|m| m.as_array())
        .map(|a| a.iter().filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
        .unwrap_or_default()
}

/// Run Basic Pitch on an audio file; returns (start, end, midi) note events.
pub fn basic_pitch(job: &Job, audio: &Path) -> Option<Vec<(f32, f32, i32)>> {
    let tool = find_tool("basic-pitch")?;
    let dir = workspace_dir().join("transcribe");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    if !run_logged(job, helper(tool).arg(&dir).arg(audio).arg("--save-note-events")) {
        return None;
    }
    let csv = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().map_or(false, |x| x == "csv"))?;
    let text = std::fs::read_to_string(csv).ok()?;
    let mut notes = vec![];
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        if c.len() >= 3 {
            if let (Ok(s), Ok(e), Ok(m)) = (c[0].trim().parse::<f32>(), c[1].trim().parse::<f32>(), c[2].trim().parse::<f32>()) {
                notes.push((s, e, m as i32));
            }
        }
    }
    Some(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run with BANDTOOLS_TEST_SONG=<wav or YouTube link> cargo test -- --ignored split_
    #[test]
    #[ignore]
    fn split_song_into_timeline_stems() {
        let src = std::env::var("BANDTOOLS_TEST_SONG").expect("BANDTOOLS_TEST_SONG");
        let job = Job::default();
        split_stems(job.clone(), src, "htdemucs_6s".into(), 48000.0);
        std::thread::sleep(std::time::Duration::from_millis(200));
        while job.state() == JobState::Running {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let i = job.inner.lock().unwrap();
        for l in &i.log {
            eprintln!("{l}");
        }
        assert_eq!(i.state, JobState::Done);
        assert!(i.stems.iter().any(|(n, s)| n == "guitar" && !s.is_empty()));
    }

    /// Run with BANDTOOLS_TEST_MODEL=<model> cargo test -- --ignored ask_
    #[test]
    #[ignore]
    fn ask_local_ai_for_json() {
        let model = std::env::var("BANDTOOLS_TEST_MODEL").unwrap_or_else(|_| "qwen2.5:7b".into());
        let job = Job::default();
        ensure_ollama(&job).unwrap();
        ensure_model(&job, &model).unwrap();
        let v = ollama_json(&model, &crate::ai::tone_prompt("90s grunge rhythm")).unwrap();
        eprintln!("{v}");
        assert!(v.get("amp").is_some());
    }
}
