use std::{fs, time::Duration };
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use futures::stream::StreamExt;
use ferropress::Settings;
use async_std::task::spawn;

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    version: String,
}

struct Response {
    status: String,         // "200 OK", "404 Not Found", etc...
    contents: String,
}


impl Request {
    async fn from_stream(mut stream: &TcpStream) -> Request {
        let mut buf = [0; 1024];
        stream.read(&mut buf).await.unwrap();

        let s = String::from_utf8(buf.to_vec()).unwrap();
        let mut parts = s.split_whitespace();
        let method = parts.next().unwrap();
        let path = parts.next().unwrap();
        let version = parts.next().unwrap();

        Request {
            method: method.to_string(),
            path: path.to_string(),
            version: version.to_string(),
        }  
    }
}


impl Response {
    fn fmt_as_bytes(&self) -> Vec<u8> {
        let version = "HTTP/1.1";
        let len = self.contents.len();
        let status = &self.status;
        let contents = &self.contents;
        let response = format!("{version} {status}\r\nContent-Length: {len}\r\n\r\n{contents}");

        response.as_bytes().to_vec()
    }
}

async fn test_view() -> Response {
    async_std::task::sleep(Duration::from_secs(5)).await;
    let status = String::from("200 OK");
    let contents = fs::read_to_string("./templates/index.html").unwrap();
    Response{status, contents}
}

async fn index_view() -> Response {
    let status = String::from("200 OK");
    let contents = fs::read_to_string("./templates/index.html").unwrap();
    Response{status, contents}
}

async fn resource_view(path: &str) -> Response {
    let status = String::from("200 OK");
    let contents = fs::read_to_string(path).unwrap();
    Response{status, contents}
}

async fn route(request: Request) -> Response {
    match &request.path[..] {
        "/test" => test_view().await,
        "/" => index_view().await,
        _ => resource_view(&request.path).await,
    }
}

#[async_std::main]
async fn main() {
    const SETTINGS_FILE_PATH: &str = "./settings.json";
    let settings = Settings::load_from_file(SETTINGS_FILE_PATH).expect("failed to load settings module; exiting!");
    
    let host = format!("{}:{}", settings.host, settings.port);
    println!("Listening on {}", host);
    let listener = TcpListener::bind(host).await.unwrap();
    listener
        .incoming()
        .for_each_concurrent(None, |tcpstream| async move {
            let tcpstream = tcpstream.unwrap();
            spawn(handle_connection(tcpstream));
        }).await;
}

async fn handle_connection(mut stream: TcpStream) {
    let request = Request::from_stream(&stream).await;
    println!("{:?}", request);

    let response = route(request).await.fmt_as_bytes();

    stream.write_all(&response[..]).await.unwrap();
    stream.flush().await.unwrap();
}




