//! Sparkle appcasts: an RSS feed whose items describe releases.

use super::version;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Release {
    /// Build number (compared with CFBundleVersion).
    pub version: String,
    /// What people see ("3.7.6"); falls back to `version`.
    pub short_version: Option<String>,
    pub url: String,
    pub length: Option<u64>,
    pub ed_signature: Option<String>,
    pub minimum_os: Option<String>,
    pub maximum_os: Option<String>,
    pub channel: Option<String>,
    pub notes_html: Option<String>,
    pub notes_link: Option<String>,
    pub hardware: Option<String>,
    pub os: Option<String>,
    pub critical: bool,
}

fn entity(name: &str) -> String {
    match name {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        n => n
            .strip_prefix("#x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .or_else(|| n.strip_prefix('#').and_then(|d| d.parse().ok()))
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default(),
    }
}

pub fn parse(xml: &str) -> Vec<Release> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut cur: Option<Release> = None;
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();

    loop {
        let ev = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(e) => e,
        };
        match ev {
            Event::Start(e) => {
                let name = e.name().as_ref().to_owned();
                open(&e, &name, &mut cur);
                path.push(name);
                text.clear();
            }
            // Self-closing (<enclosure …/>): no End follows, so it never joins `path`.
            Event::Empty(e) => {
                let name = e.name().as_ref().to_owned();
                open(&e, &name, &mut cur);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::CData(t) => text.push_str(&t.into_inner()),
            Event::GeneralRef(r) => text.push_str(&entity(&r)),
            Event::End(e) => {
                let name = e.name().as_ref().to_owned();
                if let Some(r) = cur.as_mut() {
                    let v = text.trim().to_owned();
                    let parent_is_item = path.len() >= 2 && path[path.len() - 2] == "item";
                    match name.as_str() {
                        "sparkle:version" if parent_is_item => r.version = v,
                        "sparkle:shortVersionString" if parent_is_item => r.short_version = Some(v),
                        "sparkle:minimumSystemVersion" => r.minimum_os = Some(v),
                        "sparkle:maximumSystemVersion" => r.maximum_os = Some(v),
                        "sparkle:channel" => r.channel = Some(v),
                        "sparkle:releaseNotesLink" => r.notes_link = Some(v),
                        "sparkle:hardwareRequirements" => r.hardware = Some(v),
                        "sparkle:criticalUpdate" => r.critical = true,
                        "description" if parent_is_item => r.notes_html = Some(v),
                        "item" => {
                            if let Some(done) = cur.take() {
                                if !done.url.is_empty() {
                                    out.push(done);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if path.last().is_some_and(|p| *p == name) {
                    path.pop();
                }
                text.clear();
            }
            _ => {}
        }
    }
    // Items whose version lives only in the title ("Version 3.7.6").
    for r in &mut out {
        if r.version.is_empty() {
            r.version = r.short_version.clone().unwrap_or_default();
        }
    }
    out.retain(|r| !r.version.is_empty());
    out
}

fn open(e: &quick_xml::events::BytesStart, name: &str, cur: &mut Option<Release>) {
    if name == "item" {
        *cur = Some(Release::default());
        return;
    }
    let (Some(r), "enclosure") = (cur.as_mut(), name) else { return };
    for a in e.attributes().flatten() {
        let key = a.key.as_ref().to_owned();
        let val = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|v| v.trim().to_owned()).unwrap_or_default();
        match key.as_str() {
            "url" => r.url = val,
            "length" => r.length = val.parse().ok().filter(|n| *n > 0),
            "sparkle:version" => r.version = val,
            "sparkle:shortVersionString" => r.short_version = Some(val),
            "sparkle:edSignature" => r.ed_signature = Some(val),
            "sparkle:os" => r.os = Some(val),
            _ => {}
        }
    }
}

/// The newest release this Mac can run: stable channel, OS and CPU in range.
pub fn best<'a>(releases: &'a [Release], os_version: &str, arch: &str) -> Option<&'a Release> {
    releases
        .iter()
        .filter(|r| r.channel.as_deref().is_none_or(|c| c.is_empty() || c == "release"))
        .filter(|r| r.os.as_deref().is_none_or(|o| o == "macos"))
        .filter(|r| r.minimum_os.as_deref().is_none_or(|m| !version::newer(m, os_version)))
        .filter(|r| r.maximum_os.as_deref().is_none_or(|m| !version::newer(os_version, m)))
        .filter(|r| r.hardware.as_deref().is_none_or(|h| !h.contains("arm64") || arch == "aarch64"))
        .filter(|r| r.url.starts_with("https://"))
        .max_by(|a, b| version::compare(&a.version, &b.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
<channel>
  <title>App</title>
  <item>
    <title>Version 3.6.11</title>
    <sparkle:minimumSystemVersion>12.4</sparkle:minimumSystemVersion>
    <enclosure url="https://x.test/App-3_6_11.zip" sparkle:version="3.6.11" length="45124064"
      sparkle:edSignature="SIG1" />
  </item>
  <item>
    <title>Version 3.7.6</title>
    <description><![CDATA[<h2>New</h2><p>Faster &amp; better</p>]]></description>
    <sparkle:version>376</sparkle:version>
    <sparkle:shortVersionString>3.7.6</sparkle:shortVersionString>
    <sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>
    <enclosure url="https://x.test/App-3_7_6.dmg" length="1" />
  </item>
  <item>
    <title>Beta</title>
    <sparkle:channel>beta</sparkle:channel>
    <enclosure url="https://x.test/App-4.zip" sparkle:version="400" />
  </item>
  <item>
    <title>Needs Tahoe 99</title>
    <sparkle:minimumSystemVersion>99.0</sparkle:minimumSystemVersion>
    <enclosure url="https://x.test/App-5.zip" sparkle:version="500" />
  </item>
</channel></rss>"#;

    #[test]
    fn parses_and_picks_the_right_release() {
        let all = parse(FEED);
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].ed_signature.as_deref(), Some("SIG1"));
        assert_eq!(all[0].length, Some(45124064));
        assert_eq!(all[1].version, "376");
        assert_eq!(all[1].short_version.as_deref(), Some("3.7.6"));
        assert_eq!(all[1].notes_html.as_deref(), Some("<h2>New</h2><p>Faster &amp; better</p>"));
        assert_eq!(all[2].channel.as_deref(), Some("beta"));

        let pick = best(&all, "26.6.2", "x86_64").unwrap();
        assert_eq!(pick.url, "https://x.test/App-3_7_6.dmg");
        assert_eq!(best(&all, "12.5", "x86_64").unwrap().url, "https://x.test/App-3_6_11.zip");
    }
}
