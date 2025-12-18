# XML File Watcher Docker Container

A Nix flake that builds a Docker container which monitors a directory tree for new XML files and triggers webhooks when they appear. Written in Rust for performance and reliability.

## Features

- Recursive directory monitoring using the `notify` Rust crate
- Triggers webhook on new XML files (created or moved into watched directory)
- Configurable webhook URL, method, and payload options
- Lightweight container built with Nix
- High-performance Rust implementation with async I/O

## Building

```bash
# Build the Docker image
nix build .#docker

# Load the image into Docker
docker load < result
```

## Running

### Docker

```bash
docker run -d \
  -v /path/to/watch:/watch \
  -e WEBHOOK_URL="https://your-webhook.example.com/endpoint" \
  -e WEBHOOK_METHOD="POST" \
  -e INCLUDE_CONTENT="false" \
  xml-watcher:latest
```

### Docker Compose

```yaml
version: '3.8'
services:
  xml-watcher:
    image: xml-watcher:latest
    volumes:
      - ./watched-folder:/watch
    environment:
      - WEBHOOK_URL=https://your-webhook.example.com/endpoint
      - WEBHOOK_METHOD=POST
      - INCLUDE_CONTENT=false
    restart: unless-stopped
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WATCH_DIR` | `/watch` | Directory to monitor for XML files |
| `WEBHOOK_URL` | (required) | URL to send webhook requests to |
| `WEBHOOK_METHOD` | `POST` | HTTP method for webhook requests |
| `INCLUDE_CONTENT` | `false` | Include full XML file content in payload |
| `OVERWRITE_WITH_RESPONSE` | `false` | Overwrite file with server response (requires `INCLUDE_CONTENT=true`) |
| `RUST_LOG` | - | Set log level (trace, debug, info, warn, error) |

## Webhook Payload

The webhook sends a JSON payload like this:

```json
{
  "event": "new_xml_file",
  "filepath": "/watch/subdir/example.xml",
  "filename": "example.xml",
  "timestamp": "2024-01-15T10:30:00+00:00"
}
```

Note: The `filename` field is always included in the payload.

With `INCLUDE_CONTENT=true`:

```json
{
  "event": "new_xml_file",
  "filepath": "/watch/subdir/example.xml",
  "filename": "example.xml",
  "content": "<?xml version=\"1.0\"?>...",
  "timestamp": "2024-01-15T10:30:00+00:00"
}
```

## File Overwrite Feature

When `OVERWRITE_WITH_RESPONSE=true` is set (along with `INCLUDE_CONTENT=true`), the watcher will overwrite the original XML file with the response from the webhook server. This feature has the following requirements:

- The webhook must respond with a successful HTTP status code (2xx)
- The response `Content-Type` header must contain "xml" (e.g., `text/xml` or `application/xml`)
- The response body must not be empty

When these conditions are met, the watcher will:
1. Overwrite the original file with the response content
2. Temporarily ignore watch events for that file to prevent triggering a new webhook
3. Log the overwrite operation

This is useful for scenarios where the server processes the XML and returns a modified or transformed version.

## Development

### Run locally (without Docker)

```bash
# Enter dev shell
nix develop

# Build with cargo
cargo build --release

# Or run directly with Nix
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/hook nix run

# Or run with cargo
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/hook cargo run
```

### Test the watcher

```bash
# Terminal 1: Start a simple webhook receiver
python3 -m http.server 8080

# Terminal 2: Run the watcher
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/ nix run

# Terminal 3: Create a test XML file
mkdir -p /tmp/watch
echo '<?xml version="1.0"?><test/>' > /tmp/watch/test.xml
```

The `RUST_LOG` environment variable controls logging levels (trace, debug, info, warn, error).

## License

MIT