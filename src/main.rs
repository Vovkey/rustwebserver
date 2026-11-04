use std::fs;
use std::io::{BufReader, prelude::*};
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || {
            loop {
                let job = receiver.lock().unwrap().recv();

                match job {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4);

    let shutdown_flag = Arc::new(AtomicBool::new(false));

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let flag_clone = Arc::clone(&shutdown_flag);

        pool.execute(move || {
            handle_connection(stream, &flag_clone);
        });

        if shutdown_flag.load(Ordering::SeqCst) {
            println!("Shutdown-Signal recieved. Shutting down Server...");
            break;
        }
    }
}

fn handle_connection(mut stream: TcpStream, shutdown_flag: &Arc<AtomicBool>) {
    let buf_reader = BufReader::new(&stream);
    let request_line = match buf_reader.lines().next() {
        Some(Ok(line)) => line,
        _ => return,
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let uri = parts[1];

    if method == "GET" {
        if uri == "/sleep" {
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
        } else if uri == "/quit" {
            shutdown_flag.store(true, Ordering::SeqCst);
            let file_path = Path::new("public/quit.html");
            let contents = fs::read(file_path).unwrap_or_default();
            send_response(
                &mut stream,
                "HTTP/1.1 200 OK",
                "text/html; charset=UTF-8",
                &contents,
            );
            return;
        }
    }

    let file_path = get_file_path(uri);
    let content_type = get_content_type(&file_path);

    let status_line = if file_path.ends_with("404.html") {
        "HTTP/1.1 404 NOT FOUND"
    } else {
        "HTTP/1.1 200 Ok"
    };

    let contents = fs::read(&file_path).unwrap_or_else(|_| b"<h1>404 Not Found</h1>".to_vec());

    send_response(&mut stream, status_line, content_type, &contents);
}

fn send_response(stream: &mut TcpStream, status_line: &str, content_type: &str, contents: &[u8]) {
    let length = contents.len();
    let response = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {length}\r\n\r\n"
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(contents);
    let _ = stream.flush();
}

fn get_file_path(request_uri: &str) -> PathBuf {
    let path_str = if request_uri == "/" {
        "/index.html"
    } else {
        request_uri
    };

    let clean_path = path_str.trim_start_matches('/');
    let public_dir = Path::new("public");
    let requested_path = public_dir.join(clean_path);

    if let Ok(canonical_public) = public_dir.canonicalize() {
        if let Ok(canonical_requested) = requested_path.canonicalize() {
            if canonical_requested.starts_with(&canonical_public) && canonical_requested.is_file() {
                return canonical_requested;
            }
        }
    }
    public_dir.join("404.html")
}

fn get_content_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=UTF-8",
        Some("css") => "text/css; charset=UTF-8",
        Some("js") => "application/javascript; charset=UTF-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "text/plain; charset=UTF-8",
    }
}
