import os
import zipfile

# Define target zip path
zip_path = "workspace_rust_completo.zip"

# Re-create the code and ensure it is saved in a accessible web-root location if sandbox has specific constraints, 
# but usually sandbox:/filename is available if created.
# Let's ensure the file structure is correct and cleanly compressed.

def create_workspace():
    # File contents
    cargo_root = """[workspace]
members = [
    "crate_api",
    "crate_core",
    "crate_db"
]
resolver = "2"
"""

    cargo_api = """[package]
name = "crate_api"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
crate_core = { path = "../crate_core" }
"""

    main_api = """use axum::{routing::get, Json, Router};
use std::net::SocketAddr;
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("🚀 Servidor HTTP iniciado em http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root_handler() -> String {
    let core_msg = crate_core::services::execute_business_logic();
    format!("=== API Gateway ===\\n\\n{}", core_msg)
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "workspace": "multi-crate-rust" }))
}
"""

    cargo_core = """[package]
name = "crate_core"
version = "0.1.0"
edition = "2021"

[dependencies]
crate_db = { path = "../crate_db" }
"""

    lib_core = """pub mod services {
    pub fn execute_business_logic() -> String {
        let db_data = crate_db::repositories::save_to_db();
        format!("-> [Crate Core] Processando lógica de negócio...\\n{}", db_data)
    }
}
"""

    cargo_db = """[package]
name = "crate_db"
version = "0.1.0"
edition = "2021"

[dependencies]
"""

    lib_db = """pub mod repositories {
    pub fn save_to_db() -> String {
        "-> [Crate DB] Sucesso: Dados simulados persistidos no PostgreSQL!".to_string()
    }
}
"""

    # Writing files dynamically
    files = {
        "Cargo.toml": cargo_root,
        "crate_api/Cargo.toml": cargo_api,
        "crate_api/src/main.rs": main_api,
        "crate_core/Cargo.toml": cargo_core,
        "crate_core/src/lib.rs": lib_core,
        "crate_db/Cargo.toml": cargo_db,
        "crate_db/src/lib.rs": lib_db,
    }

    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
        for path, content in files.items():
            # Create dummy directory structure inside zip
            zipf.writestr(path, content)

create_workspace()
print(f"Zip created successfully at {os.path.abspath(zip_path)}, size: {os.path.getsize(zip_path)}")
