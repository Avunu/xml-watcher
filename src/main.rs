use chrono::Utc;
use log::{error, info};
use notify::{Event, RecursiveMode, Result as NotifyResult, Watcher};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
struct WebhookPayload {
    event: String,
    filepath: String,
    filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    timestamp: String,
}

struct Config {
    watch_dir: PathBuf,
    webhook_url: String,
    webhook_method: String,
    include_filename: bool,
    include_content: bool,
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
        
        let include_filename = env::var("INCLUDE_FILENAME")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase() == "true";
        
        let include_content = env::var("INCLUDE_CONTENT")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";
        
        Ok(Config {
            watch_dir,
            webhook_url,
            webhook_method,
            include_filename,
            include_content,
        })
    }
}

fn is_xml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
}

async fn trigger_webhook(config: &Config, filepath: PathBuf) {
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
    
    info!("Starting XML file watcher...");
    info!("  Watch directory: {}", config.watch_dir.display());
    info!("  Webhook URL: {}", config.webhook_url);
    info!("  Webhook method: {}", config.webhook_method);
    info!("  Include filename: {}", config.include_filename);
    info!("  Include content: {}", config.include_content);
    
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
                if matches!(event.kind, notify::EventKind::Create(_) | notify::EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() && is_xml_file(&path) {
                            // Small delay to ensure file is fully written
                            let config_clone = Config {
                                watch_dir: config.watch_dir.clone(),
                                webhook_url: config.webhook_url.clone(),
                                webhook_method: config.webhook_method.clone(),
                                include_filename: config.include_filename,
                                include_content: config.include_content,
                            };
                            tokio::spawn(async move {
                                sleep(Duration::from_millis(500)).await;
                                trigger_webhook(&config_clone, path).await;
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
