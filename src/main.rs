mod http;
mod metrics;
mod thread_pool;

use http::{HttpRequest, get_content_type, get_file_path, send_response};
use metrics::{ActiveConnectionGuard, ServerMetrics};
use thread_pool::ThreadPool;

use std::fs;
use std::io::BufReader;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::time::Instant;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4);

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let metrics = Arc::new(ServerMetrics::new());

    println!("Started Server on http://127.0.0.1:7878");

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

            let json_body = format!(
                r#"{{"total_requests": {}, "active_connections": {}, "not_found_errors": {}}}"#,
                total, active, not_found
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
