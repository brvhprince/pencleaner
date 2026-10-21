use super::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Smaller duplicates aren't worth a review row.
const MIN_DUP: u64 = 512 * 1024;
/// Bytes hashed from each end before committing to a full read.
const EDGE: u64 = 16 * 1024;

type Digest = [u8; 32];

fn edges(path: &Path) -> Option<Digest> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; EDGE as usize];
    let n = f.read(&mut buf).ok()?;
    h.update(&buf[..n]);
    if len > 2 * EDGE {
        f.seek(SeekFrom::End(-(EDGE as i64))).ok()?;
        let n = f.read(&mut buf).ok()?;
        h.update(&buf[..n]);
    }
    Some(*h.finalize().as_bytes())
}

fn full(path: &Path) -> Option<Digest> {
    let mut h = blake3::Hasher::new();
    h.update_reader(File::open(path).ok()?).ok()?;
    Some(*h.finalize().as_bytes())
}

/// Splits a group of same-sized files by `digest`, keeping only buckets with 2+ files.
fn split<'a>(group: Vec<&'a Rec>, digest: fn(&Path) -> Option<Digest>, ctl: &ScanControl) -> Vec<Vec<&'a Rec>> {
    if ctl.is_cancelled() {
        return vec![];
    }
    let keyed: Vec<(Digest, &Rec)> = group
        .par_iter()
        .filter_map(|r| {
            ctl.set_current(&r.path);
            digest(&r.path).map(|d| (d, *r))
        })
        .collect();
    let mut buckets: HashMap<Digest, Vec<&Rec>> = HashMap::new();
    for (d, r) in keyed {
        buckets.entry(d).or_default().push(r);
    }
    buckets.into_values().filter(|b| b.len() > 1).collect()
}

/// Lower is a better copy to keep: outside Downloads/Desktop, not named like a copy,
/// then the oldest, then the shortest path.
fn keep_rank(r: &Rec) -> (u8, i64, usize) {
    let in_folder = |name: &str| r.path.components().any(|c| c.as_os_str() == name);
    let mut penalty = 0;
    if in_folder("Downloads") {
        penalty += 2;
    } else if in_folder("Desktop") {
        penalty += 1;
    }
    let name = r.path.file_stem().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.ends_with(" copy") || name.contains(" copy ") || name.ends_with(')') {
        penalty += 1;
    }
    (penalty, r.created.unwrap_or(i64::MAX), r.path.as_os_str().len())
}

pub(super) fn find(recs: &[Rec], ctl: &ScanControl) -> Vec<FileSet> {
    let mut by_len: HashMap<u64, Vec<&Rec>> = HashMap::new();
    let mut seen_inodes = HashSet::new();
    for r in recs.iter().filter(|r| r.len >= MIN_DUP) {
        // Hard links are one file under two names, not duplicates.
        if seen_inodes.insert((r.dev, r.ino)) {
            by_len.entry(r.len).or_default().push(r);
        }
    }
    let same_size: Vec<Vec<&Rec>> = by_len.into_values().filter(|g| g.len() > 1).collect();
    let same_edges: Vec<Vec<&Rec>> = same_size.into_par_iter().flat_map_iter(|g| split(g, edges, ctl)).collect();
    let identical: Vec<Vec<&Rec>> = same_edges.into_par_iter().flat_map_iter(|g| split(g, full, ctl)).collect();

    let mut sets: Vec<FileSet> = identical
        .into_iter()
        .map(|mut group| {
            group.sort_by_key(|r| keep_rank(r));
            let keep = group[0].path.to_string_lossy().into_owned();
            // Copies sharing blocks with the kept file (or with each other) free them once
            // at most: count each distinct physical location other than the kept one's.
            let kept_at = fsutil::physical_offset(&group[0].path);
            let mut counted = HashSet::new();
            let reclaimable = group[1..]
                .iter()
                .filter(|r| match fsutil::physical_offset(&r.path) {
                    Some(at) => Some(at) != kept_at && counted.insert(at),
                    None => true,
                })
                .map(|r| r.alloc)
                .sum();
            FileSet { id: format!("dup:{keep}"), files: group.iter().map(|r| r.info()).collect(), keep, reclaimable }
        })
        .collect();
    sets.sort_unstable_by_key(|s| std::cmp::Reverse(s.reclaimable));
    sets
}
