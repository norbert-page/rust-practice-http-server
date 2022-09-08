//! Basic multi-threaded *HTTP server* alongside custom [*Thread-pool library*](../threadpool/index.html), 
//! both written as a rust exercise, purposefully without using any non-standard libraries.
//!
//! Server listens on all interfaces, both IPv6 and IPv4, on port **7878** and shutdowns after **3** connection attempts.
//! Supported paths are:
//! - `/`: immediately returns `hello.html` (response code 200)
//! - `/sleep`: sleeps for 15 seconds and then returns hello.html (response code 200)
//! - any other path: returns `404.html` (response code 404)
//! 
//! In case of error reading any of the source files, `404.html` or `hello.html`, the server
//! responds with `500 Internal Server Error` (response code 500).

use std::{
    fs,
    io::{prelude::*, BufReader},
    net::{Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    thread,
    time::Duration,
};

use threadpool::ThreadPool;

fn main() {
    const PORT: u16 = 7878; // "rust" on a phone keyboard
    const SEQUENTIAL_CONNECTIONS_LIMIT: usize = 3; // cumulative number of connection attempts after which server shutdowns (to test shutdown)
    const THREAD_POOL_SIZE: usize = 4; // number of threads in a thread-pool

    // not self-evident, but this will also listen on Ipv4
    let all_addrs: SocketAddr = (Ipv6Addr::UNSPECIFIED, PORT).into();

    // bind to the socket
    let listener = TcpListener::bind(all_addrs).unwrap();

    // create a pool with a given number of threads
    let pool = ThreadPool::new(THREAD_POOL_SIZE);

    // iterate over connection attempts
    // streams are dropped when connection is closed
    // we label the loop just for rust practice
    'process_connection_attempts: for stream in
        listener.incoming().take(SEQUENTIAL_CONNECTIONS_LIMIT)
    {
        if stream.is_err() {
            eprintln!("{}", stream.unwrap_err());
            continue 'process_connection_attempts;
        } else {
            pool.execute(|| {
                handle_connection(stream.unwrap());
            });
        }
    }

    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);

    let request_line = buf_reader.lines().next();

    // don't process connection if there is a problem with the request
    // example: input cannot be interpreted as correct UTF8 (ASCII characters have the
    // same bytecodes in UTF8)
    let request_line = match request_line {
        Some(Ok(line)) => line,
        _ => return,
    };

    // we only support three cases
    let (mut status_line, filename) = match &request_line[..] {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "hello.html"),

        "GET /sleep HTTP/1.1" => {
            // artificial delay to simulate prolonged processing
            thread::sleep(Duration::from_secs(15));
            ("HTTP/1.1 200 OK", "hello.html")
        }

        _ => ("HTTP/1.1 404 NOT FOUND", "404.html"),
    };

    let contents = fs::read_to_string(filename).unwrap_or_else(|err| {
        status_line = "HTTP/1.1 500 Internal Server Error";
        eprintln!("Error reading {filename}: {err}");
        "500 Internal Server Error".to_string()
    });
    let length = contents.len();

    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

    let mut result = stream.write_all(response.as_bytes());
    if result.is_ok() {
        result = stream.flush();
    }

    if result.is_err() {
        eprintln!("{}", result.unwrap_err());
    }
}
