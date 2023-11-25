# Welcome
This guide is made for you to understand and familar with **SpeedRs**.
## Core HTTP Server
```rust
fn main() {
    // Create new server
    let mut server = HttpServer::new(HttpServerMode::SingleThread, "127.0.0.1:3000");

    // Insert a request handler
    server.insert_handler(|req, mut res| {
        res.set_status(HttpStatusStruct(200, "OK"));
        res.insert_header(String::from("Content-Type"), String::from("text/plain"));
        res.text(String::from("Hello from server!"));
        Ok((req, res))
    });
    
    // Start listening
    server.listen(|| {
        println!("Server is listening at http://127.0.0.1:3000");
    });
}
```
## Serve Static Files
To serve static files, first you need to import `ServeStatic` from module `utils`:
```rust
use speed_rs_core::utils::ServeStatic;
```
Call `serve_static()` within `HttpServer` like this:
```rust
fn main() {
    // Create new server
    let mut server = HttpServer::new(HttpServerMode::SingleThread, "127.0.0.1:3000");

    // Server will look into assets folder. You can access the files using /assets/<your-file> route
    server.serve_static(Some(String::from("assets")));
    
    // Start listening
    server.listen(|| {
        println!("Server is listening at http://127.0.0.1:3000");
    });
}
```
## Routing
**SpeedRs** provides you `Router` and `Route` structs as well as `Routing` trait to help you with routing.
```rust
use speed_rs_core::{HttpServer, HttpServerMode, HttpStatusStruct};
use speed_rs_core::utils::{Router, Route, Routing};

fn main() {
    // Create new server
    let mut server = HttpServer::new(HttpServerMode::SingleThread, "127.0.0.1:3000");

    // Create new Router
    let mut router = Router::new();

    // Define a route
    router.define_route(Route::get("/test/", |req, mut res| {
        res.set_status(HttpStatusStruct(200, "OK"));
        res.insert_header(String::from("Content-Type"), String::from("text/plain"));
        res.text(String::from("Hello from /test/"));
        Ok((req, res))
    }));

    // Add router to server
    server.insert_router(router);
    
    // Start listening
    server.listen(|| {
        println!("Server is listening at http://127.0.0.1:3000");
    });
}
```