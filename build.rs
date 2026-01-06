use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct Config {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    taiga: TaigaConfig,
}

#[derive(Deserialize)]
struct TaigaConfig {
    codename: String,
}

fn main() {
    // 1. Read Cargo.toml
    let toml_str = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml");

    // 2. Parse it using the structs above
    let config: Config = toml::from_str(&toml_str).expect("Failed to parse Cargo.toml");

    // 3. Extract the codename
    let codename = config.package.metadata.taiga.codename;

    // 4. Emit the instruction to cargo: "Set this env var for the compiler"
    println!("cargo:rustc-env=CODENAME={}", codename);

    // 5. Tell Cargo to rerun this script if Cargo.toml changes
    println!("cargo:rerun-if-changed=Cargo.toml");
}
