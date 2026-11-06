mod http;
mod metrics;
mod thread_pool;

use http::{get_content_type, get_file_path, send_response, HttpRequest};
use metrics::{ActiveConnectionGuard, ServerMetrics};
use thread_pool::ThreadPool;

use std::fs;
use std::io::BufReader;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4);

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let metrics
}
