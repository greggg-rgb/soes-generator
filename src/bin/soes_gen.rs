use soes_generator::{emit, model::Project};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out: Option<PathBuf> = None;
    let mut project_path: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        if arg == "--out" {
            out = Some(PathBuf::from(it.next().ok_or("--out requires a value")?));
        } else {
            project_path = Some(PathBuf::from(arg));
        }
    }
    let (project_path, out_dir) = match (project_path, out) {
        (Some(p), Some(o)) => (p, o),
        _ => return Err("usage: soes-gen <project.json> --out <dir>".into()),
    };

    let project = Project::from_json_file(&project_path)?;
    let emitted = emit(&project, &out_dir)?;
    for path in emitted.paths {
        println!("{}", path.display());
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
