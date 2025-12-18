use chrono::Utc;
use log::{error, info, warn};
use notify::{Event, RecursiveMode, Result as NotifyResult, Watcher};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{mpsc::channel, Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

// Duration to keep files in the ignore list after overwriting them
// This prevents triggering new webhook events when we modify the file
const IGNORE_DURATION_SECS: u64 = 2;

#[derive(Debug, Serialize, Deserialize)]
struct WebhookPayload {
    event: String,
    filepath: String,
    filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    timestamp: String,
}

#[derive(Debug, Clone)]
struct Config {
    watch_dir: PathBuf,
    webhook_url: String,
    webhook_method: String,
    include_content: bool,
    overwrite_with_response: bool,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let watch_dir = env::var("WATCH_DIR")
            .unwrap_or_else(|_| "/watch".to_string())
            .into();
        
        let webhook_url = env::var("WEBHOOK_URL")
            .map_err(|_| "WEBHOOK_URL environment variable is required".to_string())?;
        
        let webhook_method = env::var("WEBHOOK_METHOD")
            .unwrap_or_else(|_| "POST".to_string());
        
        let include_content = env::var("INCLUDE_CONTENT")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        
        let overwrite_with_response = env::var("OVERWRITE_WITH_RESPONSE")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        
        Ok(Config {
            watch_dir,
            webhook_url,
            webhook_method,
            include_content,
            overwrite_with_response,
        })
    }
}

fn is_xml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
}

async fn trigger_webhook(config: &Config, filepath: PathBuf, ignore_list: Arc<Mutex<HashSet<PathBuf>>>) {
    let filename = filepath
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_string();
    
    info!("New XML file detected: {}", filepath.display());
    
    let content = if config.include_content {
        match tokio::fs::read_to_string(&filepath).await {
            Ok(c) => Some(c),
            Err(e) => {
                error!("Failed to read file content: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    let payload = WebhookPayload {
        event: "new_xml_file".to_string(),
        filepath: filepath.display().to_string(),
        filename,
        content,
        timestamp: Utc::now().to_rfc3339(),
    };
    
    info!("Sending webhook...");
    
    let client = Client::new();
    let request_builder = match config.webhook_method.to_uppercase().as_str() {
        "GET" => client.get(&config.webhook_url),
        "PUT" => client.put(&config.webhook_url),
        "PATCH" => client.patch(&config.webhook_url),
        "DELETE" => client.delete(&config.webhook_url),
        _ => client.post(&config.webhook_url),
    };
    
    match request_builder
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                info!("  Webhook sent successfully (HTTP {})", status.as_u16());
                
                // Handle overwriting the file with response if enabled
                let should_overwrite_with_response = |config: &Config| {
                    config.overwrite_with_response && config.include_content
                };

                if should_overwrite_with_response(&config) {
                    let content_type = response.headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    
                    // Check if content type is appropriate (text/xml or application/xml)
                    // Accept content types that start with these prefixes (may include charset parameter)
                    let is_xml = content_type.starts_with("text/xml") 
                        || content_type.starts_with("application/xml");
                    
                    if is_xml {
                        match response.text().await {
                            Ok(response_body) => {
                                if !response_body.is_empty() {
                                    // Add file to ignore list before writing
                                    {
                                        let mut ignore = ignore_list.lock().unwrap();
                                        ignore.insert(filepath.clone());
                                    }
                                    
                                    match tokio::fs::write(&filepath, &response_body).await {
                                        Ok(_) => {
                                            info!("  File overwritten with response content");
                                            // Keep file in ignore list for a short time
                                            let ignore_list_clone = Arc::clone(&ignore_list);
                                            let filepath_clone = filepath.clone();
                                            tokio::spawn(async move {
                                                sleep(Duration::from_secs(IGNORE_DURATION_SECS)).await;
                                                let mut ignore = ignore_list_clone.lock().unwrap();
                                                ignore.remove(&filepath_clone);
                                            });
                                        }
                                        Err(e) => {
                                            error!("  Failed to overwrite file: {}", e);
                                            // Remove from ignore list on failure
                                            let mut ignore = ignore_list.lock().unwrap();
                                            ignore.remove(&filepath);
                                        }
                                    }
                                } else {
                                    warn!("  Response body is empty, not overwriting file");
                                }
                            }
                            Err(e) => {
                                error!("  Failed to read response body: {}", e);
                            }
                        }
                    } else {
                        warn!("  Response content-type '{}' is not XML, not overwriting file", content_type);
                    }
                }
            } else {
                let body = response.text().await.unwrap_or_default();
                error!("  Webhook failed (HTTP {}): {}", status.as_u16(), body);
            }
        }
        Err(e) => {
            error!("  Webhook request failed: {}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            std::process::exit(1);
        }
    };
    
    if !config.watch_dir.exists() {
        eprintln!("ERROR: Watch directory '{}' does not exist", config.watch_dir.display());
        std::process::exit(1);
    }
    
    // Warn if overwrite is enabled without content inclusion
    if config.overwrite_with_response && !config.include_content {
        warn!("OVERWRITE_WITH_RESPONSE is enabled but INCLUDE_CONTENT is disabled. File overwrite will not work without including content in the webhook.");
    }
    
    info!("Starting XML file watcher...");
    info!("  Watch directory: {}", config.watch_dir.display());
    info!("  Webhook URL: {}", config.webhook_url);
    info!("  Webhook method: {}", config.webhook_method);
    info!("  Include content: {}", config.include_content);
    info!("  Overwrite with response: {}", config.overwrite_with_response);
    
    // Create an ignore list for files we've just modified
    let ignore_list: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    
    let (tx, rx) = channel();
    
    let mut watcher = match notify::recommended_watcher(move |res: NotifyResult<Event>| {
        if let Ok(event) = res {
            tx.send(event).ok();
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ERROR: Failed to create watcher: {}", e);
            std::process::exit(1);
        }
    };
    
    if let Err(e) = watcher.watch(&config.watch_dir, RecursiveMode::Recursive) {
        eprintln!("ERROR: Failed to watch directory: {}", e);
        std::process::exit(1);
    }
    
    loop {
        match rx.recv() {
            Ok(event) => {
                // Only handle Create events to avoid duplicates (matches bash script behavior)
                if matches!(event.kind, notify::EventKind::Create(_)) {
                    for path in event.paths {
                        if path.is_file() && is_xml_file(&path) {
                            // Check if this file is in the ignore list
                            let should_ignore = {
                                let ignore = ignore_list.lock().unwrap();
                                ignore.contains(&path)
                            };
                            
                            if should_ignore {
                                info!("Ignoring file event for recently modified file: {}", path.display());
                                continue;
                            }
                            
                            // Small delay to ensure file is fully written
                            let config_clone = config.clone();
                            let ignore_list_clone = Arc::clone(&ignore_list);
                            tokio::spawn(async move {
                                sleep(Duration::from_millis(500)).await;
                                trigger_webhook(&config_clone, path, ignore_list_clone).await;
                            });
                        }
                    }
                }
            }
            Err(e) => {
                error!("Watch error: {}", e);
            }
        }
    }
}
