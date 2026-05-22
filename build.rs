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

        if let Some(lv) = val.get("language_version") {
            for field in &["name", "token", "default", "supported"] {
                assert!(
                    lv.get(field).is_some(),
                    "templates/{}/stack.json : language_version.{} manquant",
                    name,
                    field
                );
            }
            let token = lv.get("token").and_then(|v| v.as_str()).unwrap_or_else(|| {
                panic!(
                    "templates/{}/stack.json : language_version.token doit être une string",
                    name
                )
            });

            let dockerfile_path = path.join("Dockerfile");
            let dockerfile = fs::read_to_string(&dockerfile_path)
                .unwrap_or_else(|_| panic!("Impossible de lire templates/{}/Dockerfile", name));
            let placeholder = format!("__{}__", token);
            assert!(
                dockerfile.contains(&placeholder),
                "templates/{}/Dockerfile : placeholder '{}' attendu mais introuvable",
                name,
                placeholder
            );

            let default = lv
                .get("default")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "templates/{}/stack.json : language_version.default doit être une string",
                        name
                    )
                });
            let supported = lv
                .get("supported")
                .and_then(|v| v.as_array())
                .unwrap_or_else(|| {
                    panic!(
                        "templates/{}/stack.json : language_version.supported doit être un array",
                        name
                    )
                });
            assert!(
                supported
                    .iter()
                    .any(|v| v.get("version").and_then(|x| x.as_str()) == Some(default)),
                "templates/{}/stack.json : default '{}' absent de supported",
                name,
                default
            );
        }
    }
}
