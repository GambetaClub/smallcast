//! Timeout-bounded subprocess execution.
//!
//! smallcast shells out to `docker`, `ps`, `lsof`, and `git`. Any of these
//! can hang indefinitely: `docker ps` blocks when the daemon is starting,
//! shutting down, or wedged; `lsof` stalls on stale network mounts; `git`
//! push/pull stalls on an unreachable remote. A bare `Command::output()`
//! waits forever on that subprocess — and because synchronous Tauri commands
//! run on the main thread, one hung child froze the entire app (webview,
//! hotkey, and tray "Quit" all stopped responding).
//!
//! `output_with_timeout` bounds the wait: it spawns the child, drains its
//! pipes on dedicated reader threads (so the child never blocks on a full
//! 64K pipe buffer), polls `try_wait`, and kills the child once the deadline
//! passes. stdin is always `/dev/null` so a child can never block reading
//! input that will never come.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Run `cmd`, capturing stdout + stderr, killing the child if it runs past
/// `timeout`. Returns the real `Output` on a clean exit (any status), or an
/// `Err` describing a spawn failure or a timeout.
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Result<Output, String> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;

    // Drain both pipes on their own threads so a chatty child (e.g. `lsof`
    // listing every socket) can't deadlock against a full pipe buffer while
    // we sit in the poll loop.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stdout_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stderr_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_reader.join().unwrap_or_default();
                let stderr = err_reader.join().unwrap_or_default();
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_reader.join();
                    let _ = err_reader.join();
                    return Err(format!(
                        "timed out after {}s",
                        timeout.as_secs().max(1)
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_reader.join();
                let _ = err_reader.join();
                return Err(format!("wait failed: {e}"));
            }
        }
    }
}

/// Convenience wrapper for the "just give me stdout, or nothing" callers
/// (`docker ps`, `ps`, `lsof`). Returns `None` on spawn failure, timeout, or
/// a non-zero exit — every caller already treats those identically (empty
/// list), so the call sites stay one line.
pub fn capture_stdout(cmd: Command, timeout: Duration) -> Option<Vec<u8>> {
    match output_with_timeout(cmd, timeout) {
        Ok(out) if out.status.success() => Some(out.stdout),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_quick_output() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let out = capture_stdout(cmd, Duration::from_secs(5)).unwrap();
        assert_eq!(String::from_utf8_lossy(&out).trim(), "hello");
    }

    #[test]
    fn kills_on_timeout() {
        let mut cmd = Command::new("sleep");
        cmd.arg("10");
        let start = Instant::now();
        let result = capture_stdout(cmd, Duration::from_millis(200));
        assert!(result.is_none());
        // Should return promptly after the deadline, not after the full sleep.
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn non_zero_exit_is_none() {
        let mut cmd = Command::new("false");
        assert!(capture_stdout(cmd, Duration::from_secs(5)).is_none());
    }
}
