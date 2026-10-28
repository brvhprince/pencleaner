//! Version ordering the way Sparkle does it: "1.10" > "1.9", "1.0" > "1.0b2",
//! "1.0.1" > "1.0".

use std::cmp::Ordering;

#[derive(Debug, PartialEq)]
enum Part {
    Num(u64),
    Word(String),
}

fn parts(v: &str) -> Vec<Part> {
    let mut out = Vec::new();
    for chunk in v.split(|c: char| !c.is_ascii_alphanumeric()).filter(|c| !c.is_empty()) {
        let mut cur = String::new();
        let mut digits = None;
        for c in chunk.chars() {
            let d = c.is_ascii_digit();
            if digits.is_some_and(|was| was != d) {
                out.push(part(&cur, digits.unwrap()));
                cur.clear();
            }
            digits = Some(d);
            cur.push(c);
        }
        if let Some(d) = digits {
            out.push(part(&cur, d));
        }
    }
    out
}

fn part(s: &str, digits: bool) -> Part {
    if digits {
        Part::Num(s.parse().unwrap_or(u64::MAX))
    } else {
        Part::Word(s.to_ascii_lowercase())
    }
}

pub fn compare(a: &str, b: &str) -> Ordering {
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(&pb) {
        let o = match (x, y) {
            (Part::Num(x), Part::Num(y)) => x.cmp(y),
            (Part::Word(x), Part::Word(y)) => x.cmp(y),
            // A number beats a word at the same position: 1.0.1 > 1.0b.
            (Part::Num(_), Part::Word(_)) => Ordering::Greater,
            (Part::Word(_), Part::Num(_)) => Ordering::Less,
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    // The longer one wins only if it continues with a number (1.0.1 > 1.0);
    // a trailing word marks a pre-release (1.0b2 < 1.0).
    match pa.len().cmp(&pb.len()) {
        Ordering::Greater => match &pa[pb.len()] {
            Part::Num(_) => Ordering::Greater,
            Part::Word(_) => Ordering::Less,
        },
        Ordering::Less => match &pb[pa.len()] {
            Part::Num(_) => Ordering::Less,
            Part::Word(_) => Ordering::Greater,
        },
        Ordering::Equal => Ordering::Equal,
    }
}

pub fn newer(candidate: &str, installed: &str) -> bool {
    compare(candidate, installed) == Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_like_sparkle() {
        assert!(newer("1.10", "1.9"));
        assert!(newer("3.7.6", "3.6.11"));
        assert!(newer("1.0.1", "1.0"));
        assert!(newer("1.0", "1.0b2"));
        assert!(newer("1.0b3", "1.0b2"));
        assert!(newer("27.1", "26.5"));
        assert!(!newer("1.0", "1.0"));
        assert!(!newer("2.0 (1234)", "2.0 (1234)"));
        assert_eq!(compare("1.2", "1.2.0"), Ordering::Less);
    }
}
