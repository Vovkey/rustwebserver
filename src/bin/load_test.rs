use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_address = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:7878".to_string());

    let total_requests: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);

    let concurrent_threads = 20;
    let requests_per_thread = total_requests / concurrent_threads;
    let actual_total = requests_per_thread * concurrent_threads;

    println!(
        "Starting concurrency load test against http://{}...",
        target_address
    );
    println!(
        "Target requests: {} (distributed across {} threads)",
        actual_total, concurrent_threads
    );

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let start_time = Instant::now();
    let mut handles = vec![];

    for _ in 0..concurrent_threads {
        let success_clone = Arc::clone(&success_count);
        let error_clone = Arc::clone(&error_count);
        let target_clone = target_address.clone();

        let handle = thread::spawn(move || {
            for i in 0..requests_per_thread {
                let uri = if i % 2 == 0 { "/api/metrics" } else { "/" };
                let request = format!(
                    "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                    uri
                );

                match TcpStream::connect(&target_clone) {
                    Ok(mut stream) => {
                        if stream.write_all(request.as_bytes()).is_ok() {
                            let mut buffer = [0; 512];
                            if stream.read(&mut buffer).is_ok() {
                                success_clone.fetch_add(1, Ordering::SeqCst);
                            } else {
                                error_clone.fetch_add(1, Ordering::SeqCst);
                            }
                        } else {
                            error_clone.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(_) => {
                        error_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start_time.elapsed();
    let successes = success_count.load(Ordering::SeqCst);
    let errors = error_count.load(Ordering::SeqCst);

    println!("\n=================================");
    println!("LOAD TEST RESULTS:");
    println!("================================");
    println!("Duration:          {:?}", duration);
    println!("Successful reqs:   {}", successes);
    println!("Failed reqs:       {}", errors);
    println!(
        "Throughput:        {:.2} req/sec",
        total_requests as f64 / duration.as_secs_f64()
    );
    println!("================================")
}
