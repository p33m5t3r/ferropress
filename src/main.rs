use std::{
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    fs,
};

use ferropress::Settings;

fn main() {
    const SETTINGS_FILE_PATH: &str = "./settings.json";
    let settings = Settings::load_from_file(SETTINGS_FILE_PATH).expect("failed to load settings module; exiting!");
    
    let host = format!("{}:{}", settings.host, settings.port);
    let listener = TcpListener::bind(host).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    // println!("Request: {:#?}", http_request);
    let status_line = "HTTP/1.1 200 OK";
    let contents = fs::read_to_string("./templates/index.html").unwrap();
    println!("{:?}", contents);
    let length = contents.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
    stream.write_all(response.as_bytes()).unwrap();
}
