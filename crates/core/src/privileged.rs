//! The few operations that need root, batched behind one standard macOS password prompt
//! (`do shell script … with administrator privileges`). Callers validate every path
//! first; this module only quotes and runs.

use std::process::Command;

/// Wraps a shell word in single quotes.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Escapes text for an AppleScript string literal.
fn applescript_quote(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"")
}

/// Builds the AppleScript for `commands`; split out so the quoting is testable.
pub(crate) fn script(commands: &[Vec<String>], prompt: &str) -> String {
    // `;` not `&&`: one failure must not skip the rest. Callers check results on disk.
    let shell = commands
        .iter()
        .map(|argv| argv.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(" ; ");
    format!(
        "do shell script \"{}\" with administrator privileges with prompt \"{}\" without altering line endings",
        applescript_quote(&shell),
        applescript_quote(prompt)
    )
}

pub fn run_as_admin(commands: &[Vec<String>], prompt: &str) -> Result<(), String> {
    run_as_admin_output(commands, prompt, None).map(|_| ())
}

/// Like `run_as_admin`, returning what the commands printed. `limit` gives up (and kills
/// the prompt) if nobody answers in time.
pub fn run_as_admin_output(commands: &[Vec<String>], prompt: &str, limit: Option<std::time::Duration>) -> Result<String, String> {
    if commands.is_empty() {
        return Ok(String::new());
    }
    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script(commands, prompt))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let (mut out_pipe, mut err_pipe) = (child.stdout.take().ok_or("no stdout")?, child.stderr.take().ok_or("no stderr")?);
    // Drain both pipes on threads: a big result (sfltool prints thousands of lines) would
    // otherwise fill the pipe and stall the child.
    let out_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out_pipe, &mut b);
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err_pipe, &mut b);
        b
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if limit.is_some_and(|l| started.elapsed() > l) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("No password was entered in time.".into());
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    };
    let out = out_t.join().unwrap_or_default();
    let err = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
    if status.success() {
        return Ok(String::from_utf8_lossy(&out).into_owned());
    }
    if err.contains("-128") {
        Err("Administrator password was cancelled".into())
    } else {
        Err(err.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quotes_hostile_paths() {
        let s = super::script(
            &[vec!["/bin/mv".into(), "/Applications/It's \"Fun\" $(rm -rf ~).app".into(), "/Users/me/.Trash/x".into()]],
            "Say \"hi\"",
        );
        assert_eq!(
            s,
            r#"do shell script "'/bin/mv' '/Applications/It'\\''s \"Fun\" $(rm -rf ~).app' '/Users/me/.Trash/x'" with administrator privileges with prompt "Say \"hi\"" without altering line endings"#
        );
    }
}
