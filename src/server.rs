/// Dev server for `odin-ssg serve`.
///
/// Serves the output directory over HTTP and watches the source directory
/// for changes, triggering automatic rebuilds.
use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Start the dev server.
///
/// - Serves `out_dir` on `127.0.0.1:<port>`.
/// - Watches `source_dir` for changes; on any change, calls `rebuild_fn`
///   then resumes serving.
pub fn serve(
    source_dir: &Path,
    out_dir: &Path,
    port: u16,
    rebuild_fn: impl Fn() -> Result<()> + Send + 'static,
) -> Result<()> {
    let out_dir = out_dir.to_path_buf();
    let source_dir = source_dir.to_path_buf();
    let addr = format!("127.0.0.1:{}", port);

    println!("🌐 Serving at http://{}", addr);
    println!("👀 Watching {} for changes…", source_dir.display());

    // Spawn file-watcher thread
    let (tx, rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        if let Err(e) = watch_and_rebuild(&source_dir, tx, rebuild_fn) {
            eprintln!("Watcher error: {e}");
        }
    });

    // HTTP server in main thread
    let server = tiny_http::Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Cannot bind to {}: {}", addr, e))?;

    for request in server.incoming_requests() {
        // Drain any pending rebuild notifications (non-blocking)
        while rx.try_recv().is_ok() {}

        handle_request(request, &out_dir);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// File watcher
// ─────────────────────────────────────────────────────────────────────────────

fn watch_and_rebuild(
    source_dir: &Path,
    tx: mpsc::Sender<()>,
    rebuild_fn: impl Fn() -> Result<()>,
) -> Result<()> {
    let (notify_tx, notify_rx) = mpsc::channel::<Result<Event, notify::Error>>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.send(res);
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    watcher.watch(source_dir, RecursiveMode::Recursive)?;

    let mut last_rebuild = std::time::Instant::now();
    const DEBOUNCE: Duration = Duration::from_millis(300);

    loop {
        match notify_rx.recv() {
            Ok(Ok(_event)) => {
                // Debounce: only rebuild if enough time has passed
                if last_rebuild.elapsed() >= DEBOUNCE {
                    print!("🔄 Change detected — rebuilding… ");
                    match rebuild_fn() {
                        Ok(()) => println!("✓"),
                        Err(e) => eprintln!("✗ {e}"),
                    }
                    last_rebuild = std::time::Instant::now();
                    let _ = tx.send(());
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {e}"),
            Err(_) => break, // channel closed
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP request handler
// ─────────────────────────────────────────────────────────────────────────────

fn handle_request(request: tiny_http::Request, out_dir: &Path) {
    let url = request.url().to_string();

    // Resolve URL to a file path
    let rel = url.trim_start_matches('/');
    let mut file_path = out_dir.join(rel);

    // Directory → index.html
    if file_path.is_dir() {
        file_path = file_path.join("index.html");
    }
    // Bare path without extension → path/index.html
    if !file_path.exists() && file_path.extension().is_none() {
        file_path.set_extension("html");
    }

    if file_path.exists() && file_path.is_file() {
        serve_file(request, &file_path);
    } else {
        // 404
        let body = b"<h1>404 Not Found</h1>";
        let response = tiny_http::Response::from_data(body.as_slice())
            .with_status_code(404)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
            );
        let _ = request.respond(response);
    }
}

fn serve_file(request: tiny_http::Request, path: &PathBuf) {
    let mime = mime_for(path);
    match std::fs::read(path) {
        Ok(data) => {
            let response = tiny_http::Response::from_data(data)
                .with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        mime.as_bytes(),
                    )
                    .unwrap(),
                );
            let _ = request.respond(response);
        }
        Err(e) => {
            eprintln!("Read error {}: {e}", path.display());
        }
    }
}

fn mime_for(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8".to_string(),
        Some("css") => "text/css; charset=utf-8".to_string(),
        Some("js") => "application/javascript".to_string(),
        Some("json") => "application/json".to_string(),
        Some("svg") => "image/svg+xml".to_string(),
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("ico") => "image/x-icon".to_string(),
        Some("woff2") => "font/woff2".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
