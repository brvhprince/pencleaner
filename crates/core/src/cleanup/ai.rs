use super::*;

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_dir())
}

fn model_item(cx: &Cx, path: &Path, name: String) -> Option<Item> {
    ItemSpec { path, name, kind: ItemKind::Model, mode: RemoveMode::Delete, recommended: false }.build(cx)
}

#[derive(Deserialize)]
struct Manifest {
    config: Option<Layer>,
    #[serde(default)]
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
struct Layer {
    digest: String,
}

fn files_below(dir: &Path, out: &mut Vec<PathBuf>) {
    for p in children(dir) {
        if p.is_dir() {
            files_below(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Ollama stores a model as a manifest plus content-addressed blobs that models may
/// share. Each model gets the blobs only it uses, so removing one never breaks another.
fn ollama(cx: &Cx) -> Vec<Item> {
    let root = env_dir("OLLAMA_MODELS").unwrap_or_else(|| cx.p(".ollama/models"));
    let manifests_root = root.join("manifests");
    let mut files = Vec::new();
    files_below(&manifests_root, &mut files);

    let mut models: Vec<(PathBuf, String, Vec<String>)> = Vec::new();
    let mut refs: HashMap<String, usize> = HashMap::new();
    for f in files {
        let Ok(bytes) = fs::read(&f) else { continue };
        let Ok(m) = serde_json::from_slice::<Manifest>(&bytes) else { continue };
        let digests: Vec<String> = m.layers.into_iter().chain(m.config).map(|l| l.digest).collect();
        for d in &digests {
            *refs.entry(d.clone()).or_default() += 1;
        }
        // manifests/<registry>/<namespace>/<model>/<tag>
        let parts: Vec<String> = f
            .strip_prefix(&manifests_root)
            .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let name = match parts.as_slice() {
            [reg, ns, model, tag] if reg == "registry.ollama.ai" && ns == "library" => format!("{model}:{tag}"),
            [reg, ns, model, tag] if reg == "registry.ollama.ai" => format!("{ns}/{model}:{tag}"),
            other => other.join("/"),
        };
        models.push((f, name, digests));
    }

    models
        .into_iter()
        .filter_map(|(manifest, name, digests)| {
            let blob = |d: &String| root.join("blobs").join(d.replacen(':', "-", 1));
            let own: Vec<PathBuf> = digests.iter().filter(|d| refs[*d] == 1).map(blob).filter(|p| p.exists()).collect();
            let shared: u64 = digests.iter().filter(|d| refs[*d] > 1).map(|d| fsutil::disk_usage(&blob(d))).sum();
            let size: u64 = own.iter().map(|p| fsutil::disk_usage(p)).sum();
            cx.ctl.add(1, size);
            if size < MIN_ITEM {
                return None;
            }
            let mut paths = own;
            paths.push(manifest.clone());
            Some(Item {
                id: format!("ollama:{name}"),
                name,
                detail: Some(format!("Ollama · {}", cx.tilde(&root))),
                path: Some(root.to_string_lossy().into_owned()),
                size,
                kind: ItemKind::Model,
                mode: RemoveMode::Delete,
                last_activity: mtime_of(&manifest),
                recommended: false,
                warning: (shared > 0).then(|| format!("Shares {} MB with other models; that part stays.", shared / 1_000_000)),
                admin: false,
                action: Action::Paths(paths),
            })
        })
        .collect()
}

pub fn models(cx: &Cx) -> Vec<Group> {
    let mut items = ollama(cx);

    let hf = env_dir("HF_HOME").unwrap_or_else(|| cx.p(".cache/huggingface"));
    for d in children(&hf.join("hub")) {
        let raw = file_name(&d);
        let pretty = raw
            .strip_prefix("models--")
            .map(|s| s.replace("--", "/"))
            .or_else(|| raw.strip_prefix("datasets--").map(|s| format!("{} (dataset)", s.replace("--", "/"))));
        if let Some(name) = pretty {
            items.extend(model_item(cx, &d, name).map(|mut i| {
                i.detail = Some(format!("Hugging Face · {}", cx.tilde(&d)));
                i
            }));
        }
    }

    // LM Studio: models/<publisher>/<model>
    for root in [cx.p(".lmstudio/models"), cx.p(".cache/lm-studio/models")] {
        for publisher in children(&root) {
            for m in children(&publisher) {
                let name = format!("{}/{}", file_name(&publisher), file_name(&m));
                items.extend(model_item(cx, &m, name).map(|mut i| {
                    i.detail = Some(format!("LM Studio · {}", cx.tilde(&m)));
                    i
                }));
            }
        }
    }

    let single_dirs: &[(&str, &str)] = &[
        ("Library/Application Support/nomic.ai/GPT4All", "GPT4All"),
        ("jan/models", "Jan"),
        ("Library/Application Support/Jan/data/models", "Jan"),
        ("Library/Containers/com.liuliu.draw-things/Data/Documents/Models", "Draw Things"),
        (".cache/whisper", "Whisper"),
    ];
    for (rel, app) in single_dirs {
        for m in children(&cx.p(rel)) {
            let is_model_file = m.is_file()
                && m.extension().is_some_and(|e| ["gguf", "bin", "ckpt", "safetensors", "pt"].contains(&e.to_string_lossy().as_ref()));
            if m.is_dir() || is_model_file {
                items.extend(model_item(cx, &m, file_name(&m)).map(|mut i| {
                    i.detail = Some(format!("{app} · {}", cx.tilde(&m)));
                    i
                }));
            }
        }
    }
    for (rel, name) in [(".diffusionbee", "DiffusionBee models"), (".cache/torch/hub", "PyTorch Hub models")] {
        items.extend(model_item(cx, &cx.p(rel), name.into()));
    }

    vec![group(
        "ai-models",
        Category::Ai,
        "Local AI Models",
        "Models downloaded for local AI tools. Large, and they download again if you need them.",
        items,
    )]
}

pub fn tool_caches(cx: &Cx) -> Vec<Group> {
    const CACHES: &[(&str, &str, bool)] = &[
        (".cache/github-copilot", "GitHub Copilot cache", true),
        (".ollama/logs", "Ollama logs", true),
        (".cache/huggingface/xet", "Hugging Face download cache", true),
        (".cache/huggingface/datasets", "Hugging Face datasets cache", false),
        (".cache/codeium", "Codeium cache", true),
        (".cache/tabnine", "Tabnine cache", true),
        (".continue/index", "Continue index", true),
    ];
    let items = CACHES
        .iter()
        .filter_map(|(rel, name, rec)| {
            let path = cx.p(rel);
            ItemSpec { path: &path, name: (*name).into(), kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: *rec }.build(cx)
        })
        .collect();
    vec![group("ai-caches", Category::Ai, "AI Tool Caches", "Indexes, logs and downloads from AI assistants and runtimes.", items)]
}
