use serde_json::{json, Value};
use std::{fs, process::Command};

fn item(name: &str, steps: usize) -> Value {
    json!({"name":name,"steps":vec![json!({"effect":0,"brightness":5,"speed":2,"select":0});steps]})
}

#[test]
fn rejects_excessive_imports_without_changing_saved_collection() {
    let dir = std::env::temp_dir().join(format!("omalecoo-budgets-{}", std::process::id()));
    fs::create_dir_all(dir.join("lecoo-gk301")).unwrap();
    let saved = dir.join("lecoo-gk301/macros.json");
    let original = json!({"macros":[item("original",1)]}).to_string();
    fs::write(&saved, &original).unwrap();
    let file = dir.join("import.json");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
            .env("XDG_STATE_HOME", &dir)
            .args(args)
            .output()
            .unwrap()
    };
    let cases = [
        json!({"macros":(0..65).map(|n|item(&n.to_string(),1)).collect::<Vec<_>>()}),
        item("long", 257),
        json!({"macros":(0..17).map(|n|item(&n.to_string(),256)).collect::<Vec<_>>()}),
    ];
    for document in cases {
        fs::write(&file, document.to_string()).unwrap();
        let output = run(&["macro", "import", file.to_str().unwrap()]);
        assert!(
            !output.status.success(),
            "oversized collection was accepted"
        );
        assert_eq!(fs::read_to_string(&saved).unwrap(), original);
    }
    fs::write(
        &file,
        format!("{}{}", " ".repeat(1024 * 1024), item("large", 1)),
    )
    .unwrap();
    assert!(!run(&["macro", "import", file.to_str().unwrap()])
        .status
        .success());
    assert_eq!(fs::read_to_string(&saved).unwrap(), original);
    // The merged result also needs a budget, even when the input fits.
    fs::write(
        &saved,
        json!({"macros":(0..64).map(|n|item(&n.to_string(),1)).collect::<Vec<_>>()}).to_string(),
    )
    .unwrap();
    fs::write(&file, item("new", 1).to_string()).unwrap();
    let before = fs::read(&saved).unwrap();
    assert!(!run(&["macro", "import", file.to_str().unwrap()])
        .status
        .success());
    assert_eq!(fs::read(&saved).unwrap(), before);
    // Oversized legacy state must not reach the eager panel model.
    fs::write(
        &saved,
        json!({"macros":(0..65).map(|n|item(&n.to_string(),1)).collect::<Vec<_>>()}).to_string(),
    )
    .unwrap();
    let listed: Value = serde_json::from_slice(&run(&["macro", "list", "--json"]).stdout).unwrap();
    assert_eq!(listed["macros"].as_array().unwrap().len(), 0);
    // Exact record/step boundaries are usable, and replacement at capacity works.
    fs::write(&saved, r#"{"macros":[]}"#).unwrap();
    fs::write(&file, json!({"macros": (0..64).map(|n| item(&n.to_string(), if n < 16 {256} else {0})).collect::<Vec<_>>()}).to_string()).unwrap();
    assert!(run(&["macro", "import", file.to_str().unwrap()])
        .status
        .success());
    fs::write(&file, item("0", 256).to_string()).unwrap();
    assert!(run(&["macro", "import", file.to_str().unwrap()])
        .status
        .success());
    let listed: Value = serde_json::from_slice(&run(&["macro", "list", "--json"]).stdout).unwrap();
    assert_eq!(listed["macros"].as_array().unwrap().len(), 64);
    fs::write(&saved, " ".repeat(1024 * 1024 + 1)).unwrap();
    let listed: Value = serde_json::from_slice(&run(&["macro", "list", "--json"]).stdout).unwrap();
    assert_eq!(listed["macros"].as_array().unwrap().len(), 0);
    fs::remove_dir_all(dir).unwrap();
}
