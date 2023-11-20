use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{spawn, JoinHandle}, collections::HashMap,
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
///     (req, res)
/// });
/// ```
pub type RequestHandleFunc = fn (HttpRequest, HttpResponse) -> (HttpRequest, HttpResponse);

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
///     (req, res)
/// });
/// ```
/// 3. Listen
/// ```rust
/// server.listen()
/// ```
pub struct HttpServer {
    mode: HttpServerMode,
    listener: TcpListener,
    handlers: Vec<RequestHandleFunc>
}

pub struct HttpRequest {
    headers: HashMap<String, String>,
    body: HashMap<String, String>,
    method: String,
    uri: String,
    version: String
}

pub struct HttpResponse {
    headers: HashMap<String, String>,
    body: HashMap<String, String>,
    status: HttpStatusStruct
}

// Implementations

impl HttpServerThreadPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "Size of thread pool must be greater than 0");

        let (sender, receiver) = mpsc::channel::<ExecutorJob>();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut executors: Vec<HttpServerThreadExecutor> = Vec::with_capacity(size);

        for i in 0..size {
            executors.push(HttpServerThreadExecutor::new(i+1, Arc::clone(&receiver)));
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

        Self { id, thread: Some(thread) }
    }
}

impl HttpServer {
    /**
     * This function extract string data from the TCP stream request
     */
    fn handle_tcp_stream(stream: TcpStream, request_handles: Arc<Vec<RequestHandleFunc>>) {

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
            (req, res) = handle(req, res);
        }

        HttpServer::write_response(stream, req, res);
    }

    /**
     * Server write the response to client
     */
    fn write_response(mut stream: TcpStream, req: HttpRequest, mut res: HttpResponse) {
        // construct response body
        let mut body_string = String::new();
        if !res.headers().contains_key("Content-Type") {
            res.insert_header(String::from("Content-Type"), String::from("text/plain"));
        }
        let data = res.body();
        let content_type = res.headers().get("Content-Type");
        match content_type.map(AsRef::as_ref) {
            Some("application/json") => {
                body_string = serde_json::to_string(data).unwrap_or(String::new());
            }
            _ => {
                body_string = String::from(if let Some(data_string) = data.get("value") { data_string } else { "" });
            }
        }
        res.insert_header(String::from("Content-Length"), String::from(format!("{}", body_string.len())));

        // construct response headlines
        let mut response_headlines = Vec::<String>::new();
        response_headlines.push(String::from(format!("{} {} {}", req.version(), res.status().0, res.status().1)));

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
        response_string.push_str(&body_string);

        // println!("Response string: {}", &response_string);

        stream.write_all(response_string.as_bytes()).unwrap();
    }

    pub fn new(mode: HttpServerMode, bind_adr: &str) -> Self {
        let listener = TcpListener::bind(bind_adr).unwrap();
        Self { mode, listener, handlers: Vec::<RequestHandleFunc>::new() }
    }

    pub fn listen(&self) {
        for stream in self.listener.incoming() {
            let stream = stream.unwrap();
            let mut handles = Vec::<RequestHandleFunc>::new();
            for handle in &self.handlers {
                handles.push(handle.clone());
            }
            let handles_arc = Arc::new(handles);
            match &self.mode {
                HttpServerMode::SingleThread => {
                    HttpServer::handle_tcp_stream(stream, Arc::clone(&handles_arc));
                }
                HttpServerMode::MultiThread(pool) => {
                    pool.execute(move || HttpServer::handle_tcp_stream(stream, Arc::clone(&handles_arc)));
                }
            }
        }
    }

    pub fn insert_handler(&mut self, handler: RequestHandleFunc) {
        &self.handlers.push(handler);
    }
}

impl HttpRequest {
    fn new(mut request_headlines: Vec<String>, request_body_string: String) -> Self {
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

        // get the Content-Type header
        let content_type = headers.get("Content-Type");
        
        let mut body = HashMap::<String, String>::new();
        
        // if let Some(content_type) = content_type {
        //     if content_type == "application/json" {
        //         body = serde_json::from_str::<HashMap<String, String>>(&request_body_string).unwrap();
        //     } else if content_type == "application/x-www-form-urlencoded" {
        //         body = serde_qs::from_str::<HashMap<String, String>>(&request_body_string).unwrap();
        //     } else {
        //         body.insert(String::from("value"), String::from(&request_body_string));
        //     }
        // }

        match content_type.map(AsRef::as_ref) {
            Some("application/json") => {
                body = if let Ok(data) = serde_json::from_str::<HashMap<String, String>>(&request_body_string) { data } else { HashMap::<String, String>::new() };
            }
            Some("application/x-www-form-urlencoded") => {
                body = if let Ok(data) = serde_qs::from_str::<HashMap<String, String>>(&request_body_string) { data } else { HashMap::<String, String>::new() };
            }
            _ => {
                body.insert(String::from("value"), String::from(&request_body_string));
            }
        }

        Self { headers, body, method, uri, version }
    }
    
    /// Retrieve the request headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Retrieve the request body as HashMap
    pub fn body(&self) -> &HashMap<String, String> {
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
        let body = HashMap::<String, String>::new();
        let status = HttpStatusStruct(404, "Not Found");

        Self { headers, body, status }
    }

    /// Insert a pair key - value to response headers (if key is already existed, replace the old value of key)
    pub fn insert_header(&mut self, key: String, value: String) {
        &self.headers.insert(key, value);
    }

    /// Retrieve the response headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Retrieve the response body as HashMap
    pub fn body(&self) -> &HashMap<String, String> {
        &self.body
    }

    /// Insert a pair key - value to response body (if key is already existed, replace the old value of key)
    pub fn insert_body(&mut self, key: String, value: String) {
        &self.body.insert(key, value);
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