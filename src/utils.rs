use std::{collections::HashMap, fs::{self, File}, error::Error};

use mime_guess::{MimeGuess};

use crate::{HttpRequest, HttpServer, HttpStatusStruct, HttpResponse, RequestHandleFunc, RequestErrorHandleFunc};

/// Stand alone function for breaking `HttpRequest` into path and params
/// ```rust
/// server.insert_handler(|req, res| {
///     let (path, params) = break_request_uri(&req);
///     Ok((req, res))
/// });
/// ```
pub fn break_request_uri(req: &HttpRequest) -> (String, HashMap<String, String>) {
    let uri = req.uri();
    let parts: Vec<&str> = uri.split('?').collect();
    let mut params = HashMap::<String, String>::new();
    let path = String::from(if let Some(path) = parts.get(0) { path } else { "/" });
    if parts.len() >= 2 {
        let pairs: Vec<&str> = parts[1].split('&').collect();
        for pair in pairs {
            let key_val: Vec<&str> = pair.split('=').collect();
            params.insert(String::from(key_val[0]), String::from(if let Some(val) = key_val.get(1) { val } else { "" }));
        }
    }
    (path, params)
}

/// Provide more details for `HttpRequest`
/// ```rust
/// server.insert_handler(|req, res| {
///     let path = req.path();
///     let params = req.params();
///     Ok((req, res))
/// });
/// ```
pub trait MoreDetailsRequest {
    /// Get request's path
    fn path(&self) -> String;

    /// Get request's parameters
    fn params(&self) -> HashMap<String, String>;
}

impl MoreDetailsRequest for HttpRequest {
    fn path(&self) -> String {
        break_request_uri(&self).0
    }

    fn params(&self) -> HashMap<String, String> {
        break_request_uri(&self).1
    }
}

/// Provide `HttpServer` the ability to serve static files
/// ```rust
/// server.serve_static(None);      // Default folder is "public"
/// server.serve_static(Some(String::from("your_dir")));
/// ```
pub trait ServeStatic {
    /// Serve files in the `root_dir` folder. Default root dir is `public`.
    fn serve_static(&mut self, root_dir: Option<String>);
}

impl ServeStatic for HttpServer {
    fn serve_static(&mut self, root_dir: Option<String>) {
        let roor_dir = root_dir.unwrap_or(String::from("public"));
        self.insert_handler(move |req, mut res| {
            let path = req.path();
            let prefix = format!("/{}", &roor_dir);
            if let Some(index) = path.find(&prefix) {
                if index == 0 {
                    let path = String::from(&path[prefix.len()..path.len()]);
                    let file_path = format!("{}{}", &roor_dir, &path);
                    if let Ok(file) = File::open(&file_path) {
                        if let Ok(metadata) = file.metadata() {
                            if metadata.is_file() {
                                let data = fs::read(&file_path).unwrap_or(Vec::new());
                                let guess = MimeGuess::from_path(&file_path);
                                res.set_status(HttpStatusStruct(200, "OK"));
                                res.insert_header(String::from("Content-Type"), guess.first_or(mime_guess::mime::TEXT_PLAIN).to_string());
                                res.bytes(data);
                            }
                        }
                    }
                }
            }
            Ok((req, res))
        });
    }
}

/// Route definition
pub struct Route(String, RequestHandleFunc);

impl Route {
    pub fn all<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(handler))
    }

    pub fn get<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(move |req, res| {
            if req.method() == "GET" {
                handler(req, res)
            } else {
                Ok((req, res))
            }
        }))
    }

    pub fn post<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(move |req, res| {
            if req.method() == "POST" {
                handler(req, res)
            } else {
                Ok((req, res))
            }
        }))
    }
    
    pub fn put<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(move |req, res| {
            if req.method() == "PUT" {
                handler(req, res)
            } else {
                Ok((req, res))
            }
        }))
    }

    pub fn patch<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(move |req, res| {
            if req.method() == "PATCH" {
                handler(req, res)
            } else {
                Ok((req, res))
            }
        }))
    }

    pub fn delete<F>(path: &str, handler: F) -> Self
            where F: Fn(HttpRequest, HttpResponse) -> Result<(HttpRequest, HttpResponse), (HttpRequest, HttpResponse, Box<dyn Error>)> + Send + Sync + 'static {
        Self(String::from(path), Box::new(move |req, res| {
            if req.method() == "DELETE" {
                handler(req, res)
            } else {
                Ok((req, res))
            }
        }))
    }
}

/// A standard router provides basic routing support.
/// ```rust
/// let mut router = Router::new();
/// // define a route to handle request when client call GET /test/
/// router.define_route(Route::get("/test/", |req, res| {
///     res.insert_header("Content-Type".to_string(), "text/plain".to_string());
///     res.set_status(HttpStatusStruct(200, "OK"));
///     res.text(String::from("GET /test/"));
///     Ok((req, res))
/// }));
/// ```
/// Be mindful of the define order of the routes, for example:
/// ```rust
/// router.define_route(Route::all("/test/", |req, res| {...}));
/// router.define_route(Route::get("/test/", |req, res| {...}));    // This will be called again if client request with a GET method
/// ```
/// Therefore, you should be careful when define routes.
pub struct Router {
    routes: Vec<Route>
}

impl Router {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn define_route(&mut self, route: Route) {
        self.routes.push(route);
    }
}

/// Provide `HttpServer` the `insert_router()` function.
/// ```rust
/// let mut router = Router::new();
/// 
/// // Begin defining routes
/// ...
/// // End defining routes
/// 
/// server.insert_router(router);
/// ```
pub trait Routing {
    fn insert_router(&mut self, router: Router);
}

impl Routing for HttpServer {
    fn insert_router(&mut self, router: Router) {
        self.insert_handler(move |req, res| {
            let path = req.path();
            let mut routes = router.routes.iter();
            loop {
                if let Some(route) = routes.next() {
                    if route.0 == path {
                        break route.1(req, res);
                    }
                } else {
                    break Ok((req, res));
                }
            }
        });
    }
}