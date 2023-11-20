use speed_rs::core::{HttpStatusStruct, HttpServer, HttpServerMode, HttpServerThreadPool};

fn main() {
    let mut server = HttpServer::new(HttpServerMode::MultiThread(HttpServerThreadPool::new(2)), "127.0.0.1:3000");
    server.insert_handler(|mut req, mut res| {
        res.set_status(HttpStatusStruct(200, "Hello"));
        res.text(String::from(req.body()));
        (req, res)
    });
    server.listen();
}