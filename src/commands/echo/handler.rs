use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};
use chrono::Local;
use dashmap::DashMap;
use rand::Rng;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{signal, time::sleep};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::commands::{CommandHandler, EchoCommand, Output};

// Constants
const MAX_DELAY_MS: u64 = 100;
const BODY_LIMIT_BYTES: usize = 2048;
const LOG_DATE_FORMAT: &str = "%Y/%m/%d %H:%M:%S";

pub struct EchoHandler {
    listen_addr: String,
    response_text: String,
    debug: bool,
    stats: Arc<DashMap<String, AtomicU64>>,
}
impl CommandHandler for EchoHandler {
    /// Main execution method - starts the web server with graceful shutdown
    async fn execute(self) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(self.debug);
        output.stderr(&format!(
            "Starting echo server on {} with response text: {}",
            self.listen_addr, self.response_text
        ));

        let bind_addr = self.listen_addr.clone();
        let handler = Arc::new(self);
        let app = build_router(handler.clone());

        handler
            .start_server(app, &bind_addr)
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        Ok(output)
    }
}

impl EchoHandler {
    /// Constructor - creates a new Handler instance
    pub fn new(cmd: EchoCommand) -> Self {
        EchoHandler {
            listen_addr: cmd.listen,
            response_text: cmd.text,
            debug: cmd.debug,
            stats: Arc::new(DashMap::new()),
        }
    }

    async fn start_server(
        &self,
        app: Router,
        bind_addr: &str,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let mut output = Output::new(self.debug);
        let listener = tokio::net::TcpListener::bind(bind_addr).await?;
        let local_addr = listener.local_addr()?;
        output.stderr(&format!("\tEcho listening on: {}", local_addr));

        let server = axum::serve(listener, app);

        tokio::select! {
            result = server => {
                result.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            },
            _ = Self::shutdown_signal() => {
                output.stderr("\nShutdown signal received, stopping server...");
            }
        }
        self.print_stats(&mut output).await;
        Ok(output)
    }

    /// Print request statistics
    async fn print_stats(&self, output: &mut Output) {
        output.stderr("\n=== Request Stats ====");
        if self.stats.is_empty() {
            output.stderr("No requests processed");
            return;
        }

        for entry in self.stats.iter() {
            let path = entry.key();
            let count = entry.value().load(Ordering::SeqCst);
            println!("Path: {}, Requests: {}", path, count);
        }
    }

    /// Handle shutdown signals (Ctrl+C, SIGTERM)
    async fn shutdown_signal() {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }
}

impl EchoHandler {}

struct RequestInfo {
    method: String,
    path: String,
    host: String,
    user_agent: String,
}

impl RequestInfo {
    fn extract(request: &Request) -> Self {
        Self {
            method: request.method().to_string(),
            path: request.uri().path().to_string(),
            host: Self::get_header_value(request, "host"),
            user_agent: Self::get_header_value(request, "user-agent"),
        }
    }

    fn get_header_value(request: &Request, header_name: &str) -> String {
        request
            .headers()
            .get(header_name)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("-")
            .to_string()
    }
}

fn build_router(handler: Arc<EchoHandler>) -> Router {
    Router::new()
        .fallback(echo_handler)
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn_with_state(
                    handler.clone(),
                    logging_middleware,
                ))
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(handler)
}

async fn logging_middleware(
    State(handler): State<Arc<EchoHandler>>,
    request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let request_info = RequestInfo::extract(&request);

    let (debug_log, request) = if handler.debug {
        handle_debug_logging(request).await
    } else {
        (None, request)
    };

    let response = next.run(request).await;
    let duration = start_time.elapsed();

    log_request(&request_info, &response, duration);

    if let Some(debug_output) = debug_log {
        println!("{}", debug_output);
    }

    response
}
async fn echo_handler(
    State(handler): State<Arc<EchoHandler>>,
    request: Request,
) -> Response<String> {
    // Random timeout between 0 and MAX_DELAY_MS (like the Go version)
    let timeout_ms = rand::thread_rng().gen_range(0..=MAX_DELAY_MS);
    sleep(Duration::from_millis(timeout_ms)).await;

    // Update stats
    update_request_stats(&handler.stats, request.uri().path());

    // Create response
    Response::builder()
        .status(StatusCode::OK)
        .body(handler.response_text.clone())
        .expect("Failed to build response")
}

fn update_request_stats(stats: &DashMap<String, AtomicU64>, path: &str) {
    stats
        .entry(path.to_string())
        .or_insert_with(|| AtomicU64::new(0))
        .fetch_add(1, Ordering::SeqCst);
}
fn log_request(info: &RequestInfo, response: &Response, duration: Duration) {
    println!(
        "{} {} - \"{} {}\" {} \"{}\" {}ms",
        Local::now().format(LOG_DATE_FORMAT),
        info.host,
        info.method,
        info.path,
        response.status().as_u16(),
        info.user_agent,
        duration.as_millis(),
    );
}

async fn handle_debug_logging(request: Request) -> (Option<String>, Request) {
    let (parts, body) = request.into_parts();
    let request_for_dump = Request::from_parts(parts.clone(), body);
    let debug_output = dump_request(request_for_dump).await;
    let restored_request = Request::from_parts(parts, axum::body::Body::empty());
    (Some(debug_output), restored_request)
}

async fn dump_request(mut request: Request) -> String {
    let mut debug_output = String::from("=====DEBUG=====\n");

    // Format the request line: METHOD PATH VERSION
    debug_output.push_str(&format!(
        "{} {} {:?}\n",
        request.method(),
        request
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or(request.uri().path()),
        request.version()
    ));

    // Format all headers: Header-Name: value
    for (name, value) in request.headers().iter() {
        if let Ok(value_str) = value.to_str() {
            debug_output.push_str(&format!("{}: {}\n", name, value_str));
        }
    }

    // Extract the body by taking ownership
    let body = std::mem::replace(request.body_mut(), axum::body::Body::empty());
    let body_bytes = axum::body::to_bytes(body, BODY_LIMIT_BYTES)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();
    debug_output.push_str(&body_str);
    debug_output.push_str("===============");
    debug_output
}
