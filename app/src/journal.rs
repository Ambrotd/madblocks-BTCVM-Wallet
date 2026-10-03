//! A small log of what happened and what went wrong, for support:
//! `logs\wallet.log` beside the settings. It holds only what the window could
//! show anyway (errors, transaction ids, addresses), never a key, and stays
//! under two files of a megabyte each.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static DIR: OnceLock<PathBuf> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());
/// The last unexpected failure (a panic), for the window to report.
static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

const MAX_BYTES: u64 = 1 << 20;

/// Logs to `<data dir>\logs`, and records panics there.
pub fn init(data_dir: &Path) {
    let _ = DIR.set(data_dir.join("logs"));
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".into());
        let where_ = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let message = format!("{what} ({where_})");
        error(&format!("panic: {message}"));
        *LAST_PANIC.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
        default(info);
    }));
}

/// The folder the log is in.
pub fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

/// The last panic since the app started, if any.
pub fn last_panic() -> Option<String> {
    LAST_PANIC.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn warn(message: &str) {
    write("WARN", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}

fn write(level: &str, message: &str) {
    let Some(dir) = DIR.get() else { return };
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join("wallet.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, dir.join("wallet.1.log"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let line = message.replace(['\r', '\n'], " ");
        let _ = writeln!(f, "{} {level} {line}", timestamp(now()));
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unix seconds as UTC, like 2026-10-03T13:57:02Z.
fn timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest / 60 % 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_dates() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(timestamp(1_790_000_000), "2026-09-21T14:13:20Z");
    }
}
