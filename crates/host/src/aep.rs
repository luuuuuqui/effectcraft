//! After Effects project import (`file.importAep`, the `aep` feature): runs the external converter
//! `tools/aep-import/aep2effectcraft.py`, which reads the `.aep` / `.aepx` with py-aep (MIT, a
//! Python package) and writes the After Effects-style script that rebuilds the project. Needs
//! Python 3 with py-aep 0.18.1 installed (see `tools/aep-import/README.md`).
//!
//! Python: `EFFECTCRAFT_PYTHON`, else `py -3` (the Windows launcher), `python3`, `python`. The
//! converter: `EFFECTCRAFT_AEP_SCRIPT`, else `aep-import/aep2effectcraft.py` next to the
//! executable (release zips), else the repository's `tools/aep-import` (development builds).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SCRIPT: &str = "aep2effectcraft.py";

/// The [`effectcraft_engine::AepConverter`]: the generated script's code, or why there is none.
pub fn convert(path: &str) -> Result<String, String> {
    let script = find_script().ok_or_else(|| {
        format!("the converter {SCRIPT} was not found (looked next to the app in aep-import/ and in tools/aep-import/; set EFFECTCRAFT_AEP_SCRIPT)")
    })?;
    let mut tried = Vec::new();
    for (exe, pre) in pythons() {
        let mut cmd = Command::new(&exe);
        // Piped Python diagnostics can use a legacy locale; the host decodes them as UTF-8.
        cmd.env("PYTHONIOENCODING", "utf-8");
        // Read the script from stdout: simultaneous imports must not share a temporary file.
        // `--` also permits source filenames that start with a dash.
        cmd.args(&pre).arg(&script).arg("--").arg(path);
        hide_console(&mut cmd);
        let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(child) => child,
            Err(_) => {
                tried.push(exe);
                continue;
            }
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("cannot capture the converter's output".into());
        };
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("cannot capture the converter's diagnostics".into());
        };
        // Drain both pipes, with fixed limits: a large project or diagnostic must not exhaust RAM.
        let diagnostics = match std::thread::Builder::new().name("aep-diagnostics".into()).spawn(move || read_limited(stderr, 64 << 10, "diagnostics")) {
            Ok(worker) => worker,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("cannot read the converter's diagnostics: {e}"));
            }
        };
        let code = read_limited(stdout, 32 << 20, "script");
        if code.is_err() {
            let _ = child.kill();
        }
        let status = child.wait();
        let diagnostic = diagnostics.join();
        let code = code?;
        let diagnostic = diagnostic.map_err(|_| "the converter's diagnostic reader failed".to_string())??;
        let status = status.map_err(|e| format!("cannot wait for the converter: {e}"))?;
        // Windows' "App execution alias" stubs for python.exe / python3.exe exit with 9009 when no
        // Python is installed.
        if status.code() == Some(9009) {
            tried.push(exe);
            continue;
        }
        if status.success() {
            let code = String::from_utf8(code).map_err(|e| format!("the converted script is not UTF-8: {e}"))?;
            if code.trim().is_empty() {
                return Err("the converter returned an empty script".into());
            }
            return Ok(code);
        }
        let msg = String::from_utf8_lossy(&diagnostic).trim().to_string();
        return Err(if msg.is_empty() { format!("the converter failed ({status})") } else { msg });
    }
    Err(format!(
        "Python 3 was not found (tried {}). Install Python 3 and py-aep in the same environment, then set EFFECTCRAFT_PYTHON to that interpreter if it is not on PATH.",
        tried.join(", ")
    ))
}

fn read_limited(reader: impl Read, limit: u64, name: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| format!("cannot read the converted {name}: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("the converter's {name} exceeds {limit} bytes"));
    }
    Ok(bytes)
}

fn pythons() -> Vec<(String, Vec<String>)> {
    let mut v = Vec::new();
    if let Some(p) = std::env::var_os("EFFECTCRAFT_PYTHON").filter(|p| !p.is_empty()) {
        v.push((p.to_string_lossy().into_owned(), vec![]));
    }
    if cfg!(windows) {
        v.push(("py".into(), vec!["-3".into()]));
    }
    v.push(("python3".into(), vec![]));
    v.push(("python".into(), vec![]));
    v
}

fn find_script() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("EFFECTCRAFT_AEP_SCRIPT").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().as_deref().and_then(Path::parent) {
        candidates.push(dir.join("aep-import").join(SCRIPT));
        // target/release or target/debug in a checkout.
        if let Some(root) = dir.parent().and_then(Path::parent) {
            candidates.push(root.join("tools").join("aep-import").join(SCRIPT));
        }
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tools").join("aep-import").join(SCRIPT));
    candidates.into_iter().find(|p| p.is_file())
}

/// The converter is a console program; keep a window from flashing up behind the app.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

#[cfg(test)]
mod tests {
    #[test]
    fn converter_output_is_bounded_without_truncating_a_valid_script() {
        assert_eq!(super::read_limited(&b"abc"[..], 3, "script").unwrap(), b"abc");
        assert!(super::read_limited(&b"abcd"[..], 3, "script").unwrap_err().contains("exceeds 3 bytes"));
    }
}
