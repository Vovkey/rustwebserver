mod http;
mod metrics;
mod thread_pool;

use http::{HttpRequest, get_content_type, get_file_path, send_response};
use metrics::{ActiveConnectionGuard, ServerMetrics};
use thread_pool::ThreadPool;

use std::env;
use std::fs;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::time::Instant;

struct ServerConfig {
    address: String,
    worker_threads: usize,
    public_dir: String,
}

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let address = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:7878".to_string());
    let worker_threads = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);

    let config = ServerConfig {
        address,
        worker_threads,
        public_dir: "public".to_string(),
    };

    let listener = match TcpListener::bind(&config.address) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "Error: Couldn't bind server to addess '{}' : {}",
                config.address, e
            );
            eprintln!("Hint: Check if the port is already in use and if IP-address is correct.");
            process::exit(1);
        }
    };

    let pool = ThreadPool::new(config.worker_threads);
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let metrics = Arc::new(ServerMetrics::new());

    log::info!("Started Server on http://{}", config.address);
    log::info!("Worker Threads: {}", config.worker_threads);
    log::info!("Public Directory ./{}", config.public_dir);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let flag_clone = Arc::clone(&shutdown_flag);
        let metrics_clone = Arc::clone(&metrics);

        pool.execute(move || handle_connection(stream, &flag_clone, &metrics_clone));

        if shutdown_flag.load(Ordering::SeqCst) {
            println!("Shutdown-signal received. Shutting down Server...");
            break;
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    shutdown_flag: &Arc<AtomicBool>,
    metrics: &Arc<ServerMetrics>,
) {
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
        log::warn!("Failed to set read timeout: {}", e);
        return;
    }

    let start_time = Instant::now();
    let req_count = metrics.total_requests.fetch_add(1, Ordering::SeqCst) + 1;
    metrics.active_connections.fetch_add(1, Ordering::SeqCst);
    let _guard = ActiveConnectionGuard {
        counter: &metrics.active_connections,
    };

    let buf_reader = BufReader::new(&stream);
    let request = match HttpRequest::parse(buf_reader) {
        Ok(req) => req,
        Err(_) => return,
    };

    let active = metrics.active_connections.load(Ordering::SeqCst);
    println!(
        "[Req#{} | Active: {}] {} {}",
        req_count, active, request.method, request.uri
    );

    if request.method == "GET" {
        if request.uri == "/sleep" {
            thread::sleep(Duration::from_secs(5));
            let file_path = Path::new("public/index.html");
            let contents = fs::read(file_path).unwrap_or_default();
            send_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "text/html; charset=UTF-8",
                &contents,
            );
            return;
        } else if request.uri == "/quit" {
            shutdown_flag.store(true, Ordering::SeqCst);
            let file_path = Path::new("public/quit.html");
            let contents = fs::read(file_path).unwrap_or_default();
            send_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "text.html; charset=UTF-8",
                &contents,
            );
            return;
        } else if request.uri == "/stats" {
            let file_path = Path::new("public/stats.html");
            let html_template = fs::read_to_string(file_path)
                .unwrap_or_else(|_| "<h1>stats.html not found!</h1>".to_string());

            let html_body = html_template
                .replace(
                    "{{total}}",
                    &metrics.total_requests.load(Ordering::SeqCst).to_string(),
                )
                .replace(
                    "{{active}}",
                    &metrics
                        .active_connections
                        .load(Ordering::SeqCst)
                        .to_string(),
                )
                .replace(
                    "{{not_found}}",
                    &metrics.not_found_errors.load(Ordering::SeqCst).to_string(),
                );

            send_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "text/html; charset=UTF-8",
                html_body.as_bytes(),
            );
            return;
        } else if request.uri == "/api/metrics" {
            let total = metrics.total_requests.load(Ordering::SeqCst);
            let active = metrics.active_connections.load(Ordering::SeqCst);
            let not_found = metrics.not_found_errors.load(Ordering::SeqCst);
            let uptime_seconds = metrics.start_time.elapsed().as_secs();

            let json_body = format!(
                r#"{{"total_requests": {}, "active_connections": {}, "not_found_errors": {}, "uptime_seconds": {}}}"#,
                total, active, not_found, uptime_seconds
            );

            send_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "application/json; charset=UTF-8",
                json_body.as_bytes(),
            );
            return;
        }
    } else if request.method == "POST" && request.uri == "/submit" {
        if let Some(body) = &request.body {
            println!("Received POST-Body: {}", body);
        }

        let response_body = "<h1>Message Received!</h1><a href='/'>Return</a>";
        send_response(
            &mut stream,
            "HTTP/1.1 200 OK",
            "text/html; charset=UTF-8",
            response_body.as_bytes(),
        );
        return;
    }

    let file_path = get_file_path(&request.uri);
    let content_type = get_content_type(&file_path);

    let status_line = if file_path.ends_with("404.html") {
        metrics.not_found_errors.fetch_add(1, Ordering::SeqCst);
        "HTTP/1.1 404 NOT FOUND"
    } else {
        "HTTP/1.1 200 OK"
    };

    let contents = fs::read(&file_path).unwrap_or_else(|_| b"<h1>404 Not Found</h1>".to_vec());
    send_response(&mut stream, status_line, content_type, &contents);

    let duration = start_time.elapsed();
    println!(
        "[Req #{} | {}ms] {} {}",
        req_count,
        duration.as_millis(),
        request.method,
        request.uri
    );
}
