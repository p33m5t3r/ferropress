use std::{time::Duration, fmt};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use futures::stream::StreamExt;
use ferropress::Settings;
use async_std::task::spawn;
use async_std::fs;
use std::sync::{Arc, Mutex};
use log::info;
use std::collections::HashMap;


type ContentCache = Arc<Mutex<HashMap<String, Vec<u8>>>>;

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    version: String,
}

enum HttpContentType {
    Html, Css, Jpeg, Png, Icon,
}

enum HttpHeader {
    ContentType(HttpContentType),
    ContentLength(i32),
}

enum HttpStatus {
    HttpOk(i32),
    HttpErr(i32),
}

impl HttpContentType {
    fn from_str(s: &str) -> HttpContentType {
        match s {
            "html" => HttpContentType::Html,
            "css" => HttpContentType::Css,
            "jpeg" => HttpContentType::Jpeg,
            "png" => HttpContentType::Png,
            "ico" => HttpContentType::Icon,
            _ => HttpContentType::Html,
        }
    }
}

impl fmt::Display for HttpContentType {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", match self {
            HttpContentType::Html => "text/html",
            HttpContentType::Css => "text/css",
            HttpContentType::Jpeg => "image/jpeg",
            HttpContentType::Png => "image/png",
            HttpContentType::Icon => "image/x-icon",
        })
    }
}

impl fmt::Display for HttpHeader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", match self {
            HttpHeader::ContentType(s) => format!("Content-Type: {}", s),
            HttpHeader::ContentLength(n) => format!("Content-Length: {}", n),
        })
    }
}

impl fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HttpStatus::HttpOk(code) => match *code {
                200 => write!(f, "200 OK"),
                201 => write!(f, "201 Created"),
                204 => write!(f, "204 No Content"),
                _ => write!(f, "{} OK", code), // default response for other 2xx codes
            },
            HttpStatus::HttpErr(code) => match *code {
                400 => write!(f, "400 Bad Request"),
                404 => write!(f, "404 Not Found"),
                500 => write!(f, "500 Internal Server Error"),
                _ => write!(f, "{} Unknown Error", code), // default response for other error codes
            },
        }
    }
}


struct Response {
    status: HttpStatus,
    contents: Vec<u8>,
    headers: Option<Vec<HttpHeader>>,
}


impl Request {
    async fn from_stream(mut stream: &TcpStream) -> Request {
        let mut buf = [0; 1024];
        stream.read(&mut buf).await.unwrap();

        let s = String::from_utf8(buf.to_vec()).unwrap();
        info!("Raw Request:\n{}", s);
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

        let mut header_str: String = self.headers
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|header| {
                if let HttpHeader::ContentLength(_) = header {
                    None
                } else {
                    Some(format!("{}\r\n", header))
                }
            })
            .collect();
        let content_length = self.contents.len();
        header_str.push_str(&format!("Content-Length: {}\r\n\r\n", content_length));

        info!("Headers:\n{}", header_str);

        let status_line = format!("HTTP/1.1 {}\r\n", &self.status);
        let mut response_bytes = format!("{status_line}{header_str}").as_bytes().to_vec();
        response_bytes.extend_from_slice(&self.contents);

        response_bytes
    }
}

async fn test_view() -> Response {
    async_std::task::sleep(Duration::from_secs(5)).await;
    let contents =  fs::read("./templates/index.html").await.unwrap();
    Response{status: HttpStatus::HttpOk(200), contents, headers: None}
}

async fn index_view(cache: ContentCache) -> Response {
    let contents = cache.lock().unwrap().get("./templates/index.html").unwrap().clone();
    // let contents = fs::read("./templates/index.html").await.unwrap();
    let headers = Some(Vec::from([HttpHeader::ContentType(HttpContentType::Html)]));
    Response{status: HttpStatus::HttpOk(200), contents, headers} 
}

async fn resource_view(path: &str) -> Response {
    const MEDIA_TYPES: &[&str] = &["ico", "jpg", "jpeg", "png"];
    let filetype = path.split('.').last().unwrap();
    let dir = if MEDIA_TYPES.contains(&filetype) { "./media" } else { "./static" };
    let content_type = HttpContentType::from_str(filetype);
    let headers = Some(Vec::from([HttpHeader::ContentType(content_type)]));
    let full_path = format!("{}{}", dir, path);
    
    let contents = fs::read(full_path).await.unwrap();

    Response{status: HttpStatus::HttpOk(200), contents, headers}
}

async fn route(request: Request, settings: Arc<Settings>, cache: ContentCache) -> Response {
    match &request.path[..] {
        "/test" => test_view().await,
        "/" => index_view(cache).await,
        _ => resource_view(&request.path).await,
    }
}

#[async_std::main]
async fn main() {
    // export RUST_LOG=info
    env_logger::init();
    const SETTINGS_FILE_PATH: &str = "./settings.json";
    let settings = Arc::new(Settings::load_from_file(SETTINGS_FILE_PATH).expect("failed to load settings module; exiting!"));
    info!("Starting server!");
    info!("{:?}", *settings);

    let mut content_cache = HashMap::new();
    content_cache.insert(String::from("./templates/index.html"), fs::read("./templates/index.html").await.unwrap());
    let content_cache = Arc::new(Mutex::new(content_cache));

    
    let host = format!("{}:{}", settings.host, settings.port);
    println!("Listening on {}", host);
    let listener = TcpListener::bind(host).await.unwrap();
    listener
        .incoming()
        .for_each_concurrent(None, move |tcpstream| {
            let settings = Arc::clone(&settings);
            let content_cache = Arc::clone(&content_cache);
            async move {
                let tcpstream = tcpstream.unwrap();
                spawn(handle_connection(tcpstream, settings, content_cache));
            }
        }).await;
}

async fn handle_connection(mut stream: TcpStream, settings: Arc<Settings>, cache: ContentCache) {
    let request = Request::from_stream(&stream).await;
    info!("{:?}", request);

    let response = route(request, settings, cache).await.fmt_as_bytes();

    stream.write_all(&response[..]).await.unwrap();
    stream.flush().await.unwrap();
}




