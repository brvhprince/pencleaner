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
        "do shell script \"{}\" with administrator privileges with prompt \"{}\"",
        applescript_quote(&shell),
        applescript_quote(prompt)
    )
}

pub fn run_as_admin(commands: &[Vec<String>], prompt: &str) -> Result<(), String> {
    if commands.is_empty() {
        return Ok(());
    }
    let out = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script(commands, prompt))
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
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
            r#"do shell script "'/bin/mv' '/Applications/It'\\''s \"Fun\" $(rm -rf ~).app' '/Users/me/.Trash/x'" with administrator privileges with prompt "Say \"hi\"""#
        );
    }
}
