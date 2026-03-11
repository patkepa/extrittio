use utoipa::OpenApi;

fn main() {
    let spec = extrittio_backend::api::openapi::ApiDoc::openapi()
        .to_pretty_json()
        .expect("Failed to serialize OpenAPI spec");
    print!("{spec}");
}
