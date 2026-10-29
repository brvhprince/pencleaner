use super::*;

fn attachment_dirs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 7 {
        return;
    }
    for p in children(dir) {
        if !p.is_dir() {
            continue;
        }
        if file_name(&p) == "Attachments" {
            out.push(p);
        } else if !p.extension().is_some_and(|e| e == "emlx") {
            attachment_dirs(&p, depth + 1, out);
        }
    }
}

fn mail_store_version(p: &Path) -> Option<u32> {
    file_name(p).strip_prefix('V')?.parse().ok()
}

pub fn attachments(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    let mail_root = cx.p("Library/Mail");

    // Files opened from messages: Mail copies them here and never cleans up.
    let downloads = cx.p("Library/Containers/com.apple.mail/Data/Library/Mail Downloads");
    let entries = children(&downloads);
    if let (false, Some(size)) = (entries.is_empty(), cx.measure(&downloads)) {
        items.push(Item {
            id: downloads.to_string_lossy().into_owned(),
            name: "Opened attachments".into(),
            detail: Some("Copies Mail made when you opened or Quick Looked an attachment".into()),
            path: Some(downloads.to_string_lossy().into_owned()),
            size,
            kind: ItemKind::Folder,
            mode: RemoveMode::Delete,
            last_activity: mtime_of(&downloads),
            recommended: true,
            warning: None,
            admin: false,
            action: Action::Paths(entries),
        });
    }

    let stores: Vec<(u32, PathBuf)> = children(&mail_root).into_iter().filter_map(|p| Some((mail_store_version(&p)?, p))).collect();
    let current = stores.iter().map(|(v, _)| *v).max();
    for (version, store) in &stores {
        if Some(*version) == current {
            let mut dirs = Vec::new();
            attachment_dirs(store, 0, &mut dirs);
            let size: u64 = dirs.iter().filter_map(|d| cx.measure(d)).sum();
            if size >= MIN_ITEM {
                items.push(Item {
                    id: format!("{}::attachments", store.display()),
                    name: "Downloaded attachments".into(),
                    detail: Some(format!("{} folders in {}", dirs.len(), cx.tilde(store))),
                    path: Some(store.to_string_lossy().into_owned()),
                    size,
                    kind: ItemKind::Folder,
                    mode: RemoveMode::Delete,
                    last_activity: None,
                    recommended: false,
                    warning: Some("IMAP and Exchange attachments download again when you open the message. POP attachments are gone for good.".into()),
                    admin: false,
                    action: Action::Paths(dirs),
                });
            }
        } else if let Some(mut old) = (ItemSpec {
            path: store,
            name: format!("Old Mail data (V{version})"),
            kind: ItemKind::Folder,
            mode: RemoveMode::Trash,
            recommended: false,
        })
        .build(cx)
        {
            old.warning = Some("Left behind by an older macOS after Mail upgraded its database. Moved to the Trash so you can check first.".into());
            items.push(old);
        }
    }

    let mut g = group("mail", Category::Mail, "Mail Attachments", "Local copies of attachments that Mail keeps on disk.", items);
    if unreadable(&mail_root) {
        g.note = Some("Mail's folders are hidden without Full Disk Access.".into());
    }
    vec![g]
}
