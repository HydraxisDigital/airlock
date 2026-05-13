use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=templates");

    let templates = Path::new("templates");

    for entry in fs::read_dir(templates).expect("dossier templates/ introuvable") {
        let entry = entry.unwrap();
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = path.file_name().unwrap().to_string_lossy().to_string();

        // Skip _common and other underscore dirs
        if name.starts_with('_') {
            continue;
        }

        let required = ["stack.json", "Dockerfile", "post-create.sh"];
        for file in &required {
            let fp = path.join(file);
            assert!(
                fp.exists(),
                "templates/{}/{} est manquant — chaque stack doit avoir ces 3 fichiers",
                name,
                file
            );
        }

        // Validate stack.json parses as valid JSON with required fields
        let json_path = path.join("stack.json");
        let content = fs::read_to_string(&json_path)
            .unwrap_or_else(|_| panic!("Impossible de lire templates/{}/stack.json", name));

        let val: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("JSON invalide dans templates/{}/stack.json : {}", name, e));

        for field in &[
            "id",
            "label",
            "vscode_extensions",
            "audit_command",
            "tree_command",
        ] {
            assert!(
                val.get(field).is_some(),
                "templates/{}/stack.json : champ '{}' manquant",
                name,
                field
            );
        }
    }
}
