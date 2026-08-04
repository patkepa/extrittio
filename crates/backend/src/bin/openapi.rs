use utoipa::OpenApi;

fn main() {
    let spec = extrittio_backend::api::openapi::ApiDoc::openapi()
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI spec");
    if let Some(output_path) = std::env::args_os().nth(1) {
        std::fs::write(&output_path, format!("{spec}\n")).unwrap_or_else(|error| {
            panic!(
                "Failed to write OpenAPI spec to {}: {error}",
                std::path::Path::new(&output_path).display()
            )
        });
    } else {
        print!("{spec}");
    }
}
