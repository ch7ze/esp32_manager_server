use axum::{
    body::Body,
    http::StatusCode,
    response::Response,
};
use serde_json::Value;
use std::{fs, path::Path, time::UNIX_EPOCH};

pub fn get_client_hash() -> Result<String, Box<dyn std::error::Error>> {
    let hash_file = "client-hash.json";
    
    if Path::new(hash_file).exists() {
        let contents = fs::read_to_string(hash_file)?;
        let json: Value = serde_json::from_str(&contents)?;
        
        if let Some(hash) = json.get("hash").and_then(|h| h.as_str()) {
            return Ok(hash.to_string());
        }
    }
    
    println!("No client hash found, caching will not be optimal");
    Ok(String::new())
}

// ============================================================================
// SPA ROUTE HANDLER - Serviert die Haupt-HTML Datei für alle SPA-Routes
// Website-Feature: Alle URLs (/, /login, /register, etc.) zeigen die gleiche HTML
// ============================================================================

// Serviert die richtige Template-Datei basierend auf der URL
// Website-Feature: Single Page Application (SPA) Support
pub async fn handle_spa_route() -> Response<Body> {
    handle_spa_route_with_cache_control("no-cache, must-revalidate").await
}

// Serviert SPA Route mit angegebener Cache-Control
pub async fn handle_spa_route_with_cache_control(cache_control: &str) -> Response<Body> {
    handle_template_file("dest/index.html", cache_control).await
}

// Serviert eine spezifische Template-Datei
pub async fn handle_template_file(file_path: &str, cache_control: &str) -> Response<Body> {
    
    // Versuche die Template-Datei zu lesen
    match fs::read_to_string(file_path) {
        Ok(contents) => {
            // ETag für Client-seitiges Caching erstellen
            // ETag = "Entity Tag" - eindeutige Kennung für Datei-Version
            let etag = match fs::metadata(file_path) {
                Ok(metadata) => {
                    let size = metadata.len();  // Dateigröße
                    // Letzte Änderungszeit als Unix-Timestamp
                    let modified = metadata
                        .modified()
                        .unwrap_or(UNIX_EPOCH)
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    // ETag = "größe-zeitstempel"
                    format!("\"{}-{}\"", size, modified)
                }
                Err(_) => "\"default-etag\"".to_string(),
            };
            
            // HTTP Response erstellen
            Response::builder()
                .header("content-type", "text/html; charset=utf-8") // HTML mit UTF-8
                .header("etag", etag)                              // Caching-Header
                .header("cache-control", cache_control)            // Konfigurierbares Caching
                .body(Body::from(contents))                         // HTML Content
                .unwrap()
        }
        Err(err) => {
            // Fehler-Handling wenn Datei nicht gelesen werden kann
            eprintln!("Error reading {}: {}", file_path, err);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)  // HTTP 500
                .body(Body::from("Error loading content"))
                .unwrap()
        }
    }
}


// ============================================================================
// RUST KONZEPTE IN DIESER DATEI:
// 
// 1. Error Handling mit Result<T, E> und ? Operator
// 2. Pattern Matching mit match expressions
// 3. Option<T> mit .and_then() und .unwrap_or()
// 4. String vs &str (owned vs borrowed strings)
// 5. File I/O mit std::fs
// 6. HTTP Response Building mit Builder Pattern
// 7. Static Lifetimes mit &'static str
// ============================================================================