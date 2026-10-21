use super::*;
use crate::imaging::{self, Fingerprint, SIMILAR_BITS};

/// Icons and UI assets are below this; photos and screenshots are above.
const MIN_IMAGE: u64 = 40 * 1000;

fn aspect_matches(a: &Fingerprint, b: &Fingerprint) -> bool {
    if a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 {
        return true;
    }
    let ra = a.width as f64 / a.height as f64;
    let rb = b.width as f64 / b.height as f64;
    (ra - rb).abs() / ra.max(rb) < 0.1
}

/// `skip`: extra copies of exact duplicates, which the Duplicates view already covers.
pub(super) fn find(recs: &[Rec], skip: &HashSet<&Path>, ctl: &ScanControl) -> Vec<FileSet> {
    let images: Vec<&Rec> = recs
        .iter()
        .filter(|r| {
            r.len >= MIN_IMAGE
                && !skip.contains(r.path.as_path())
                && r.path
                    .extension()
                    .is_some_and(|e| imaging::IMAGE_EXTS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()))
        })
        .collect();
    let mut prints: Vec<(&Rec, Fingerprint)> = images
        .par_iter()
        .filter_map(|r| {
            if ctl.is_cancelled() {
                return None;
            }
            ctl.set_current(&r.path);
            imaging::fingerprint(&r.path).map(|f| (*r, f))
        })
        .collect();
    // Best first (most pixels, bigger file, newest), so each group forms around its best shot.
    prints.sort_by_key(|(r, f)| std::cmp::Reverse((f.width as u64 * f.height as u64, r.len, r.modified.unwrap_or(0))));

    // Star clustering: every member must be close to the group's centre, not merely to a
    // neighbour, so a series of colour-graded variants can't chain unrelated photos together.
    let n = prints.len();
    let mut taken = vec![false; n];
    let mut sets = Vec::new();
    for i in 0..n {
        if taken[i] || ctl.is_cancelled() {
            continue;
        }
        let centre = prints[i].1;
        let members: Vec<usize> = (i + 1..n)
            .into_par_iter()
            .filter(|&j| !taken[j] && centre.distance(&prints[j].1) <= SIMILAR_BITS && aspect_matches(&centre, &prints[j].1))
            .collect();
        if members.is_empty() {
            continue;
        }
        taken[i] = true;
        let mut files = Vec::with_capacity(members.len() + 1);
        for idx in std::iter::once(i).chain(members) {
            taken[idx] = true;
            let (r, f) = &prints[idx];
            let mut info = r.info();
            info.width = Some(f.width);
            info.height = Some(f.height);
            files.push(info);
        }
        let keep = files[0].path.clone();
        let reclaimable = files[1..].iter().map(|f| f.size).sum();
        sets.push(FileSet { id: format!("sim:{keep}"), files, keep, reclaimable });
    }
    sets.sort_unstable_by_key(|s| std::cmp::Reverse(s.reclaimable));
    sets
}
