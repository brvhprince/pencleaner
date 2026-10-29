//! Name, date and command-line handling for Windows' installed-programs list. Pure text
//! work with no Windows calls, so it builds and is tested on every platform.

const DAY: i64 = 86_400;

/// "Visual Studio Code (User)" → "visual studio code"; "7-Zip 23.01 (x64)" → "7-zip".
pub fn normalize(name: &str) -> String {
    let mut s = name.to_lowercase();
    if let Some(i) = s.find('(') {
        s.truncate(i);
    }
    // Stop at the version ("23.01"), but keep a first word that starts with a digit
    // ("7-Zip", "3DMark").
    let words: Vec<&str> = s
        .split_whitespace()
        .enumerate()
        .take_while(|(i, w)| *i == 0 || !w.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|(_, w)| w)
        .collect();
    // Trim separators ("App -", "Corp,"), not meaningful marks ("Notepad++").
    words.join(" ").trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | ',' | '.' | ':' | ';')).to_owned()
}

/// "Slack Technologies Inc." → "slack technologies"; good enough to compare folder names.
pub fn normalize_publisher(p: &str) -> String {
    let s = normalize(p);
    let drop = [" inc", " inc.", " llc", " ltd", " ltd.", " corporation", " corp", " corp.", " gmbh", ","];
    let mut t = s;
    for d in drop {
        if let Some(stripped) = t.strip_suffix(d) {
            t = stripped.trim().to_owned();
        }
    }
    t
}

/// "20240815" → Unix seconds.
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (s[..4].parse().ok()?, s[4..6].parse().ok()?, s[6..].parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days since 1970-01-01 (civil-from-days, proleptic Gregorian).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * DAY)
}

/// The uninstaller to start: (program, arguments). An MSI install (its product GUID
/// given) goes through msiexec with /X (uninstall), even when the entry says /I, which
/// would open a "modify" screen instead.
pub fn uninstall_command(uninstall: &str, msi_product: Option<&str>) -> (String, String) {
    if let Some(guid) = msi_product.filter(|g| g.starts_with('{')) {
        return ("msiexec.exe".into(), format!("/X{guid}"));
    }
    let cmd = uninstall.trim();
    if let Some(rest) = cmd.strip_prefix('"') {
        if let Some((exe, args)) = rest.split_once('"') {
            return (exe.to_owned(), args.trim().to_owned());
        }
    }
    // Unquoted, possibly with spaces in the path: split after ".exe".
    let lower = cmd.to_ascii_lowercase();
    if let Some(i) = lower.find(".exe") {
        let (exe, args) = cmd.split_at(i + 4);
        return (exe.trim().to_owned(), args.trim().to_owned());
    }
    match cmd.split_once(' ') {
        Some((exe, args)) => (exe.to_owned(), args.to_owned()),
        None => (cmd.to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uninstall_commands() {
        assert_eq!(
            uninstall_command(r#""C:\Program Files\App\unins000.exe" /SILENT"#, None),
            (r"C:\Program Files\App\unins000.exe".into(), "/SILENT".into())
        );
        assert_eq!(
            uninstall_command(r"C:\Program Files\App Two\uninstall.exe /S", None),
            (r"C:\Program Files\App Two\uninstall.exe".into(), "/S".into())
        );
        let guid = "{12345678-1234-1234-1234-123456789ABC}";
        assert_eq!(uninstall_command(&format!("MsiExec.exe /I{guid}"), Some(guid)), ("msiexec.exe".into(), format!("/X{guid}")));
        // Not an MSI product code: run the command as given.
        assert_eq!(uninstall_command("MsiExec.exe /X{abc}", Some("App")), ("MsiExec.exe".into(), "/X{abc}".into()));
    }

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize("7-Zip 23.01 (x64)"), "7-zip");
        assert_eq!(normalize("3DMark"), "3dmark");
        assert_eq!(normalize("Visual Studio Code (User)"), "visual studio code");
        assert_eq!(normalize("Python 3.12.4 (64-bit)"), "python");
        assert_eq!(normalize("Notepad++ (64-bit x64)"), "notepad++");
        assert_eq!(normalize_publisher("Slack Technologies Inc."), "slack technologies");
        assert_eq!(normalize_publisher("Microsoft Corporation"), "microsoft");
    }

    #[test]
    fn parses_install_dates() {
        assert_eq!(parse_date("19700102"), Some(DAY));
        assert_eq!(parse_date("20240229"), Some(19_782 * DAY));
        assert_eq!(parse_date("2024-02-29"), None);
        assert_eq!(parse_date("20241332"), None);
    }
}
