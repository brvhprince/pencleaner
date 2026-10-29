use super::*;
use crate::volumes;

fn bin(cx: &Cx, dir: &Path, name: String) -> Option<Item> {
    let entries = children(dir);
    if entries.is_empty() {
        return None;
    }
    let size = cx.measure(dir)?;
    Some(Item {
        id: dir.to_string_lossy().into_owned(),
        name,
        detail: Some(if entries.len() == 1 { "1 item".into() } else { format!("{} items", entries.len()) }),
        path: Some(dir.to_string_lossy().into_owned()),
        size,
        kind: ItemKind::Trash,
        mode: RemoveMode::Delete,
        last_activity: mtime_of(dir),
        recommended: true,
        warning: Some("Emptying the Trash can't be undone.".into()),
        admin: false,
        action: Action::Paths(entries),
    })
}

pub fn trash_bins(cx: &Cx) -> Vec<Group> {
    let uid = unsafe { libc::getuid() };
    let vols = volumes::list();
    let mut items = Vec::new();
    let home_trash = cx.p(".Trash");
    let root_name = vols.iter().find(|v| v.is_root).map_or("Mac".to_owned(), |v| v.name.clone());
    items.extend(bin(cx, &home_trash, format!("Trash on {root_name}")));
    for v in vols.iter().filter(|v| !v.is_root) {
        let dir = Path::new(&v.mount).join(".Trashes").join(uid.to_string());
        items.extend(bin(cx, &dir, format!("Trash on {}", v.name)));
    }
    let mut g = group("trash", Category::Trash, "Trash Bins", "Everything in the Trash on this Mac and connected drives.", items);
    if unreadable(&home_trash) {
        g.note = Some("The Trash is hidden without Full Disk Access.".into());
    }
    vec![g]
}
