#!/usr/bin/env bash
set -euo pipefail

WATCH_DIR="''${WATCH_DIR:-/watch}"
WEBHOOK_URL="''${WEBHOOK_URL:-}"
WEBHOOK_METHOD="''${WEBHOOK_METHOD:-POST}"
INCLUDE_FILENAME="''${INCLUDE_FILENAME:-true}"
INCLUDE_CONTENT="''${INCLUDE_CONTENT:-false}"

if [[ -z "$WEBHOOK_URL" ]]; then
    echo "ERROR: WEBHOOK_URL environment variable is required"
    exit 1
fi

if [[ ! -d "$WATCH_DIR" ]]; then
    echo "ERROR: Watch directory '$WATCH_DIR' does not exist"
    exit 1
fi

echo "Starting XML file watcher..."
echo "  Watch directory: $WATCH_DIR"
echo "  Webhook URL: $WEBHOOK_URL"
echo "  Webhook method: $WEBHOOK_METHOD"
echo "  Include filename: $INCLUDE_FILENAME"
echo "  Include content: $INCLUDE_CONTENT"

trigger_webhook() {
    local filepath="$1"
    local filename
    filename=$(basename "$filepath")
    
    echo "[$(date -Iseconds)] New XML file detected: $filepath"
    
    local json_payload
    if [[ "$INCLUDE_CONTENT" == "true" ]] && [[ -f "$filepath" ]]; then
        local content
        content=$(cat "$filepath" | jq -Rs .)
        json_payload=$(jq -n \
            --arg file "$filepath" \
            --arg name "$filename" \
            --argjson content "$content" \
            --arg time "$(date -Iseconds)" \
        '{event: "new_xml_file", filepath: $file, filename: $name, content: $content, timestamp: $time}')
    else
        json_payload=$(jq -n \
            --arg file "$filepath" \
            --arg name "$filename" \
            --arg time "$(date -Iseconds)" \
        '{event: "new_xml_file", filepath: $file, filename: $name, timestamp: $time}')
    fi
    
    echo "Sending webhook..."
    local response
    if response=$(curl -s -w "\n%{http_code}" \
        -X "$WEBHOOK_METHOD" \
        -H "Content-Type: application/json" \
        -d "$json_payload" \
        "$WEBHOOK_URL" 2>&1); then
        
        local http_code
        http_code=$(echo "$response" | tail -n1)
        local body
        body=$(echo "$response" | sed '$d')
        
        if [[ "$http_code" -ge 200 ]] && [[ "$http_code" -lt 300 ]]; then
            echo "  Webhook sent successfully (HTTP $http_code)"
        else
            echo "  Webhook failed (HTTP $http_code): $body"
        fi
    else
        echo "  Webhook request failed: $response"
    fi
}

# Use inotifywait to monitor for new XML files
# -m: monitor continuously
# -r: recursive
# -e: events to watch (create, moved_to covers new files)
# --format: output format
inotifywait -m -r -e create -e moved_to \
--format '%w%f' "$WATCH_DIR" | while read -r filepath; do
    
    # Check if it's an XML file (case insensitive)
    if [[ "$filepath" =~ \.[xX][mM][lL]$ ]]; then
        # Small delay to ensure file is fully written
        sleep 0.5
        trigger_webhook "$filepath"
    fi
done