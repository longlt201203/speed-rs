use std::{collections::HashMap, fs::{self, File}};

use mime_guess::{MimeGuess};

use crate::{HttpRequest, HttpServer, HttpStatusStruct};

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
/// server.serve_static(None);
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
            Ok((req, res))
        });
    }
}