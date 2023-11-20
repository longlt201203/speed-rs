use speed_rs::core::{HttpStatusStruct, HttpServer, HttpServerMode, HttpServerThreadPool, HttpRequest};

fn main() {
    let mut server = HttpServer::new(HttpServerMode::MultiThread(HttpServerThreadPool::new(2)), "127.0.0.1:3000");
    server.insert_handler(|mut req, mut res| {
        let uri = req.uri();
        let body = req.body();
        let headers = req.headers();
        res.set_status(HttpStatusStruct(200, "OK"));
        res.insert_header(String::from("Content-Type"), String::from("application/json"));
        res.insert_body(String::from("headers"), serde_json::to_string(headers).unwrap_or(String::new()));
        res.insert_body(String::from("body"), serde_json::to_string(body).unwrap_or(String::new()));
        (req, res)
    });
    server.listen();
}