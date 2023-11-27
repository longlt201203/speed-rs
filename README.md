# speed-rs-core
![](speedrs-logo.png)

A core HTTP Server implementation for Rust.
## Installation
Create a new Rust project using `cargo`
```shell
cargo new your-project
```
Add the package to your project
```
cargo add speed-rs-core
```
Or add the following line to the dependencies in your `Cargo.toml` file:
```
[dependencies]
...
speed-rs-core = "0.4.1"
```
Finally build the project
```
cargo build
```
Now you can use the package freely.
## How To Use
`speed-rs-core` provides just the core HTTP handling, so you will need to handle the higher-level abstractions. Below is an example of how to respond with an HTML file to the client when there is a request:
```rust
use std::fs;
use speed_rs_core::{HttpServer, HttpServerMode, HttpStatusStruct};

fn main() {
    // Create the server in single-thread mode
    let mut server = HttpServer::new(HttpServerMode::SingleThread, "127.0.0.1:3000");
    
    // Provide the request handling function
    server.insert_handler(|mut req, mut res| {
        res.set_status(HttpStatusStruct(200, "OK"));
        res.insert_header("Content-Type".to_string(), "text/html".to_string());


        // Read the HTML
        let err = match fs::read_to_string("public/index.html") {
            Ok(html) => {
                res.text(html);
                None
            },
            Err(e) => Some(e)
        };

        // Since the ownership of req and res are taken, you must return them back to the server
        if let Some(e) = err { Err((req, res, Box::new(e))) } else { Ok((req, res)) }
    });

    // Start listening for requests
    server.listen(|| {
        println!("Server is listening at http://127.0.0.1:3000");
    });
}
```
> [!NOTE]
> For more details, you can checkout this [guide](GUIDE.MD).
## Development Guide
To further develop this package and leverage the powerful features of Rust, you can implement traits like `RequestParamsExtractor` for additional functionalities:
```rust
use std::collections::HashMap;
use speed_rs_core::HttpRequest;

trait RequestParamsExtractor {
    fn params(&self) -> HashMap<String, String>;
}

impl RequestParamsExtractor for HttpRequest {
    fn params(&self) -> HashMap<String, String> {
        // Implementation code here
        HashMap::new()
    }
}

// In your server's request handler
server.insert_handler(|mut req, mut res| {
    // ...
    let params = req.params();
    // ...
    Ok((req, res))
});
```
## License
Distributed under the **MIT License**. See `LICENSE` for more information.