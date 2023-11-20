//! SpeedRs provide you a fast, efficient way to construct HTTP Server

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    panic,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{spawn, JoinHandle}, error::Error,
};

// Enums

/// HTTP server run mode
/// - `SingleThread` - run in single thread
/// - `MultiThread` - run with a thread pool (`HttpServerThreadPool`)
///
/// Example:
/// ```rust
/// let mut server = HttpServer::new(HttpServerMode::SingleThread, "127.0.0.1:3000");
/// let mut server = HttpServer::new(HttpServerMode::MultiThread(HttpServerThreadPool::new(2)), "127.0.0.1:3000");
/// ```
pub enum HttpServerMode {
    SingleThread,
    MultiThread(HttpServerThreadPool),
}

// Types

type ExecutorJob = Box<dyn FnOnce() + Send + 'static>;

/// Handle function for HTTP request.
///
/// Example:
/// ```rust
/// server.insert_handler(|mut req, mut res| {
///     res.set_status(HttpStatusStruct(200, "OK"));
///     res.set_body(String::from("value"), String::from("Hello World!"));
///     Ok(req, res)
/// });
/// ```
pub type RequestHandleFunc = fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)>;

/// Handle function for HTTP request when Error
/// 
/// Example:
/// ```rust
/// server.set_error_handler(|req, mut res, err| {
///     res.set_status(HttpStatusStruct(500, "Interal Server Error"));
///     res.text(format!("Unhandled exception: {:?}", err));
///     (req, res)
/// });
/// ```
pub type RequestErrorHandleFunc = fn(HttpRequest, HttpResponse, Box<dyn Error>) -> (HttpRequest, HttpResponse);

// Traits

// Declarations
/// HTTP status structure.
///
/// Example:
/// ```rust
/// HttpStatusStruct(200, "OK")
/// HttpStatusStruct(400, "Not Found")
/// HttpStatusStruct(500, "This is not a bug. It is a feature.")
/// ```
pub struct HttpStatusStruct(pub i32, pub &'static str);

/// Thread pool implementation for multi-thread HTTP server process.
/// ```rust
/// HttpServerThreadPool::new(4)    // 4 threads for handling requests
/// ```
pub struct HttpServerThreadPool {
    size: usize,
    executors: Vec<HttpServerThreadExecutor>,
    sender: Option<Sender<ExecutorJob>>,
}

struct HttpServerThreadExecutor {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

/// The almighty HTTP server.
///
/// Guide:
/// 1. Create the server
/// ```rust
/// let mut server = HttpServer::new(HttpServerMode::MultiThread(HttpServerThreadPool::new(2)), "127.0.0.1:3000");
/// ```
/// 2. Insert handlers
/// ```rust
/// server.insert_handler(|mut req, mut res| {
///     res.set_status(HttpStatusStruct(200, "OK"));
///     res.set_body(String::from("value"), String::from("Hello World!"));
///     Ok(req, res)
/// });
/// ```
/// 3. Listen
/// ```rust
/// server.listen(|| {
///     println!("Server is listening at http://127.0.0.1:3000");
/// });
/// ```
pub struct HttpServer {
    mode: HttpServerMode,
    listener: TcpListener,
    handlers: Vec<RequestHandleFunc>,
    error_handler: RequestErrorHandleFunc
}

pub struct HttpRequest {
    headers: HashMap<String, String>,
    body: String,
    method: String,
    uri: String,
    version: String,
}

pub struct HttpResponse {
    headers: HashMap<String, String>,
    body: String,
    status: HttpStatusStruct,
}

// Implementations

impl HttpServerThreadPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "Size of thread pool must be greater than 0");

        let (sender, receiver) = mpsc::channel::<ExecutorJob>();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut executors: Vec<HttpServerThreadExecutor> = Vec::with_capacity(size);

        for i in 0..size {
            executors.push(HttpServerThreadExecutor::new(i + 1, Arc::clone(&receiver)));
        }

        Self {
            size,
            executors,
            sender: Some(sender),
        }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

// Clean up the thread pool
impl Drop for HttpServerThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for executor in &mut self.executors {
            // println!("Shutting the executor {} down...", executor.id);

            if let Some(thread) = executor.thread.take() {
                thread.join().unwrap();
            }

            // println!("Executor {} shutted down.", executor.id);
        }
    }
}

impl HttpServerThreadExecutor {
    pub fn new(id: usize, receiver: Arc<Mutex<Receiver<ExecutorJob>>>) -> Self {
        let thread = spawn(move || loop {
            let job = receiver.lock().unwrap().recv();

            match job {
                Ok(job) => {
                    // println!("Executor {} received a job. Begin executing...", id);

                    job();

                    // println!("Executor {} finished its job.", id);
                }
                Err(_err) => {
                    // println!("{:?}", err);
                    // println!("Shutting executor down!");
                    break;
                }
            }
        });

        Self {
            id,
            thread: Some(thread),
        }
    }
}

impl HttpServer {
    /**
     * This function extract string data from the TCP stream request
     */
    fn handle_tcp_stream(stream: TcpStream, request_handles: Arc<Vec<RequestHandleFunc>>, request_error_handle: Arc<RequestErrorHandleFunc>) {
        // init reader
        let mut reader = BufReader::new(&stream);

        // read the request headlines
        let request_headlines: Vec<String> = reader
            .by_ref()
            .lines()
            .map(|line| line.unwrap())
            .take_while(|line| !line.is_empty())
            .collect();

        // find content length
        let content_length = request_headlines
            .iter()
            .find_map(|line| {
                let parts: Vec<_> = line.splitn(2, ':').collect();
                if parts[0].to_lowercase() == "content-length" {
                    parts.get(1)?.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        // read the request body
        let mut body = String::new();
        if content_length > 0 {
            reader
                .by_ref()
                .take(content_length as u64)
                .read_to_string(&mut body)
                .unwrap();
        }

        // println!("Request headlines: {:?}", request_headlines);
        // println!("Request body string: {}", &body);

        let mut req = HttpRequest::new(request_headlines, body);
        let mut res = HttpResponse::new();

        for handle in request_handles.as_ref() {
            (req, res) = match handle(req, res) {
                Ok((req, res)) => (req, res),
                Err((req, res, e)) => request_error_handle(req, res, e)
            }
        }

        HttpServer::write_response(stream, req, res);
    }

    /**
     * Server write the response to client
     */
    fn write_response(mut stream: TcpStream, req: HttpRequest, mut res: HttpResponse) {
        // construct response body
        if !res.headers().contains_key("Content-Type") {
            res.insert_header(String::from("Content-Type"), String::from("text/plain"));
        }
        res.insert_header(
            String::from("Content-Length"),
            String::from(format!("{}", res.body().len())),
        );

        // construct response headlines
        let mut response_headlines = Vec::<String>::new();
        response_headlines.push(String::from(format!(
            "{} {} {}",
            req.version(),
            res.status().0,
            res.status().1
        )));

        for header in res.headers() {
            response_headlines.push(String::from(format!("{}: {}", header.0, header.1)));
        }

        // construct response string
        let mut response_string = String::new();

        for line in response_headlines {
            response_string.push_str(&line);
            response_string.push('\n');
        }
        response_string.push('\n');
        response_string.push_str(res.body());

        // println!("Response string: {}", &response_string);

        stream.write_all(response_string.as_bytes()).unwrap();
    }

    pub fn new(mode: HttpServerMode, bind_adr: &str) -> Self {
        let listener = TcpListener::bind(bind_adr).unwrap();
        Self {
            mode,
            listener,
            handlers: Vec::<RequestHandleFunc>::new(),
            error_handler: |req, mut res, err| {
                res.set_status(HttpStatusStruct(500, "Interal Server Error"));
                res.text(format!("Unhandled exception: {:?}", err));
                (req, res)
            }
        }
    }

    pub fn listen(&self, cb: fn()) {
        cb();
        for stream in self.listener.incoming() {
            let stream = stream.unwrap();
            let mut handles = Vec::<RequestHandleFunc>::new();
            for handle in &self.handlers {
                handles.push(handle.clone());
            }
            let handles_arc = Arc::new(handles);
            let error_handle_arc = Arc::new(self.error_handler);
            match &self.mode {
                HttpServerMode::SingleThread => {
                    if let Err(e) = panic::catch_unwind(move || HttpServer::handle_tcp_stream(stream, Arc::clone(&handles_arc), Arc::clone(&error_handle_arc))) {
                        println!("Panic occurred in handle_tcp_stream()!");
                        println!("Error: {:?}", e);
                    }
                }
                HttpServerMode::MultiThread(pool) => {
                    pool.execute(move || {
                        if let Err(e) = panic::catch_unwind(move || HttpServer::handle_tcp_stream(stream, Arc::clone(&handles_arc), Arc::clone(&error_handle_arc))) {
                            println!("Panic occurred in handle_tcp_stream()!");
                            println!("Error: {:?}", e);
                        }
                    });
                }
            }
        }
    }

    pub fn insert_handler(&mut self, handler: RequestHandleFunc) {
        &self.handlers.push(handler);
    }

    /// Custom error handling function
    /// 
    /// Example:
    /// ```rust
    /// server.set_error_handler(|req, mut res, err| {
    ///     res.set_status(HttpStatusStruct(500, "Interal Server Error"));
    ///     res.text(format!("Unhandled exception: {:?}", err));
    ///     (req, res)
    /// });
    /// ```
    pub fn set_error_handler(&mut self, handler: RequestErrorHandleFunc) {
        self.error_handler = handler;
    }
}

impl HttpRequest {
    fn new(mut request_headlines: Vec<String>, body: String) -> Self {
        // get the first line out
        let first_line = request_headlines.remove(0);
        let metadata: Vec<&str> = first_line.split(" ").collect();
        let method = String::from(metadata[0]);
        let uri = String::from(metadata[1]);
        let version = String::from(metadata[2]);

        // transform header strings to headers map
        let mut headers = HashMap::<String, String>::new();
        for line in request_headlines {
            let elements: Vec<&str> = line.split(": ").collect();
            if elements.len() >= 2 {
                headers.insert(String::from(elements[0]), String::from(elements[1]));
            }
        }

        Self {
            headers,
            body,
            method,
            uri,
            version,
        }
    }

    /// Retrieve the request headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Retrieve the request body
    pub fn body(&self) -> &String {
        &self.body
    }

    /// Retrieve the request method
    pub fn method(&self) -> &String {
        &self.method
    }

    /// Retrieve the request URI
    pub fn uri(&self) -> &String {
        &self.uri
    }

    /// Retrieve the HTTP version
    pub fn version(&self) -> &String {
        &self.version
    }
}

impl HttpResponse {
    fn new() -> Self {
        let headers = HashMap::<String, String>::new();
        let body = String::new();
        let status = HttpStatusStruct(404, "Not Found");

        Self {
            headers,
            body,
            status,
        }
    }

    /// Insert a pair key - value to response headers (if key is already existed, replace the old value of key)
    pub fn insert_header(&mut self, key: String, value: String) {
        &self.headers.insert(key, value);
    }

    /// Retrieve the response headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Retrieve the response body
    pub fn body(&self) -> &String {
        &self.body
    }

    /// Set the response body text
    pub fn text(&mut self, t: String) {
        self.body = t;
    }

    /// Retrieve the response status
    pub fn status(&self) -> &HttpStatusStruct {
        &self.status
    }

    /// Set the response status
    pub fn set_status(&mut self, status: HttpStatusStruct) {
        self.status = status;
    }
}
