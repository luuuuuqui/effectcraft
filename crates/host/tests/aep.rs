//! After Effects project import (`file.importAep`, the `aep` feature). With the feature, the
//! round trip runs when `EC_TEST_AEP` names an `.aep` / `.aepx` (and Python 3 with py-aep is
//! installed); without it the test only checks the command is unavailable.

use serde_json::json;

#[test]
fn aep_import_follows_the_build_feature() {
    let mut s = effectcraft_host::session();
    if !cfg!(feature = "aep") {
        assert!(!s.is_enabled("file.importAep"));
        assert!(s.execute("file.importAep", json!({"path": "x.aep"})).is_err());
        return;
    }
    assert!(s.is_enabled("file.importAep"));
    // A missing file is an error, not a crash.
    assert!(s.execute("file.importAep", json!({"path": "/no/such/project.aep"})).is_err());
    let Some(path) = std::env::var_os("EC_TEST_AEP") else {
        println!("note: EC_TEST_AEP is not set; skipping the .aep round trip");
        return;
    };
    let r = s.execute("file.importAep", json!({"path": path.to_string_lossy()})).unwrap();
    let output = r["output"].as_str().unwrap_or_default();
    assert!(output.starts_with("Imported "), "{r}");
    let comps = s.project.items.values().filter(|i| matches!(i.kind, effectcraft_engine::project::ItemKind::Comp(_))).count();
    assert!(comps > 0, "no comps imported: {output}");
}

#[test]
fn converter_failure_preserves_the_open_document() {
    let mut s = effectcraft_host::session();
    s.execute("comp.new", json!({"name": "existing"})).unwrap();
    let before = s.project.clone();
    s.aep_converter = Some(|_| Err("synthetic converter failure".into()));
    let error = s.execute("file.importAep", json!({"path": "broken.aep"})).unwrap_err().to_string();
    assert!(error.contains("synthetic converter failure"), "{error}");
    assert_eq!(s.project, before);
}

/// The Python helper generates our own fixture; no binary or Adobe-authored test asset is shipped.
#[cfg(all(feature = "aep", not(target_arch = "wasm32")))]
#[test]
fn synthetic_aep_preserves_markers_animation_and_undo() {
    use std::path::Path;
    use std::process::Command;

    let python = std::env::var("EFFECTCRAFT_PYTHON").unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
    if !Command::new(&python).args(["-c", "import py_aep"]).status().is_ok_and(|s| s.success()) {
        eprintln!("skipping synthetic .aep round trip: Python with py-aep is required");
        return;
    }
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("effectcraft-aep-synthetic-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("synthetic.aep");
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/aep-import");
    let status = Command::new(&python)
        .args(["-c", "import sys; sys.path.insert(0, sys.argv[1]); from test_import import make_project; make_project(sys.argv[2])"])
        .arg(tools)
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let mut s = effectcraft_host::session();
    let missing = dir.join("missing-é-✓.aep");
    let diagnostic = s.aep_converter.unwrap()(missing.to_str().unwrap()).unwrap_err();
    assert!(diagnostic.contains("missing-é-✓.aep"), "{diagnostic}");
    s.execute("comp.new", json!({"name": "existing"})).unwrap();
    let existing = s.active_comp_id().unwrap();
    let reply = s.execute("file.importAep", json!({"path": source})).unwrap();
    assert!(reply["output"].as_str().unwrap().starts_with("Imported "), "{reply}");
    let imported = s.active_comp_id().unwrap();
    assert_ne!(imported, existing);
    let comp = s.project.comp(imported).unwrap();
    assert_eq!(comp.markers.len(), 1, "{reply}");
    assert_eq!(comp.markers[0].comment, "start ✓");
    assert_eq!(comp.markers[0].chapter, "chapter ✓");
    assert_eq!(comp.markers[0].duration.seconds(), 0.25);
    assert_eq!(comp.markers[0].time.seconds(), 0.5);
    assert_eq!(comp.layers.len(), 1);
    assert_eq!(comp.layers[0].markers.len(), 1);
    assert_eq!(comp.layers[0].markers[0].comment, "layer ✓");
    assert_eq!(comp.layers[0].markers[0].time.seconds(), 0.75);
    let position = comp.layers[0].props.prop("transform/position").unwrap();
    assert_eq!(position.keys.len(), 2);
    assert_eq!(position.keys[0].value.as_vec2(), [80.0, 90.0]);
    assert_eq!(position.keys[1].value.as_vec2(), [160.0, 90.0]);
    assert!(s.project.comp(existing).is_some());
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.project.comp(imported).is_none());
    assert!(s.project.comp(existing).is_some());
    std::fs::remove_dir_all(dir).unwrap();
}
