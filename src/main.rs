extern crate clap;

use chrono::{DateTime, Local};
use clap::Parser;
use std::fs::{self, File};
use std::io;
use std::io::{BufRead, Write};
use std::iter::FromIterator;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::path::PathBuf;
use std::thread;

#[derive(Parser)]
struct Opts {
    #[arg(short, long, default_value = ".")]
    dir: String,
    #[arg(short, long, default_value = "8000")]
    port: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    println!("dir: {}", opts.dir);
    println!("port: {}", opts.port);

    let bind = format!("127.0.0.1:{}", opts.port);
    println!("serve: {}", bind);

    let listener = TcpListener::bind(bind).unwrap();

    for stream in listener.incoming() {
        let base_dir = opts.dir.clone();
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream, base_dir);
                });
            }
            Err(_) => panic!("connection failed"),
        }
    }
}

fn handle_client(stream: TcpStream, base_dir: String) {
    // バッファリングを行うため BufReader を用いる
    let mut stream = io::BufReader::new(stream);

    // stream から最初の一行を読み取る
    let mut first_line = String::new();
    if let Err(err) = stream.read_line(&mut first_line) {
        panic!("error during receive a line: {}", err);
    }

    let mut params = first_line.split_whitespace();
    let method = params.next();
    let path = params.next();
    match (method, path) {
        (Some("GET"), Some(file_path)) => {
            // BufReader が所有権を持っていくため，get_mut() で内部の（可変）参照を受け取る
            get_operation(file_path, stream.get_mut(), base_dir);
        }
        _ => panic!("failed to parse"),
    }
}

#[cfg(not(target_os = "windows"))]
fn make_path(base_dir: &str, file_name: &str) -> String {
    let mut base_dir: Vec<char> = base_dir.chars().collect();

    for i in (0..base_dir.len()).rev() {
        if base_dir[i] == '/' {
            base_dir.remove(i);
        } else {
            break;
        }
    }

    let file_name = if file_name.len() == 0 {
        "/".to_string()
    } else {
        file_name.to_string()
    };

    // let file_name = if file_name.ends_with("/") {
    //     file_name + "index.html"
    // } else {
    //     file_name
    // };

    if file_name.starts_with("/") {
        format!("{}{}", String::from_iter(base_dir), file_name)
    } else {
        format!("{}/{}", String::from_iter(base_dir), file_name)
    }
}

#[cfg(target_os = "windows")]
fn make_path(base_dir: &str, file_name: &str) -> String {
    let mut base_dir: Vec<char> = base_dir.chars().collect();

    for i in (0..base_dir.len()).rev() {
        if base_dir[i] == '\\' {
            base_dir.remove(i);
        } else {
            break;
        }
    }

    let file_name = file_name.replace('/', "\\");
    let file_name = if file_name.is_empty() {
        "\\".to_string()
    } else {
        file_name
    };

    // let file_name = if file_name.ends_with('\\') {
    //     file_name + "index.html"
    // } else {
    //     file_name
    // };

    if file_name.starts_with('\\') {
        format!("{}{}", String::from_iter(base_dir), file_name)
    } else {
        format!("{}\\{}", String::from_iter(base_dir), file_name)
    }
}

fn get_mime(path: &Path) -> String {
    if let Some(ext) = path.extension() {
        match ext.to_str().unwrap_or("") {
            "js" => "application/javascript".to_string(),
            "html" | "htm" => "text/html".to_string(),
            "jpg" | "jpeg" | "jfif" | "pjpeg" | "pjp" => "image/jpeg".to_string(),
            "png" => "image/png".to_string(),
            "svg" => "image/svg+xml".to_string(),
            "gif" => "image/gif".to_string(),
            "bmp" => "image/bmp".to_string(),
            "apng" => "image/apng".to_string(),
            "tiff" | "tif" => "image/tiff".to_string(),
            "webp" => "image/webp".to_string(),
            _ => "text/plain".to_string(),
        }
    } else {
        "text/plain".to_string()
    }
}

// ディレクトリにあるファイル一覧を表示する
fn get_dir_list(path: &Path) -> String {
    let mut html = String::new();
    html.push_str("<html><head><title>Index of ");
    html.push_str(path.to_str().unwrap_or(""));
    html.push_str("</title></head><body><h1>Index of ");
    html.push_str(path.to_str().unwrap_or(""));
    html.push_str("</h1><hr><ul>");

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_str().unwrap_or("");
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                html.push_str("<li><a href=\"");
                html.push_str(file_name);
                html.push_str("/\">");
                html.push_str(file_name);
                html.push_str("/</a></li>");
            } else {
                html.push_str("<li><a href=\"");
                html.push_str(file_name);
                html.push_str("\">");
                html.push_str(file_name);
                html.push_str("</a></li>");
            }
        }
    }
    html.push_str("</ul><hr></body></html>");
    html
}

fn get_operation(file_name: &str, stream: &mut TcpStream, base_dir: String) {
    let path_str = make_path(&base_dir, file_name);
    let path = PathBuf::from(path_str.clone());
    let local_datetime: DateTime<Local> = Local::now();

    if !path.exists() {
        println!("Not Found: {} - {}", &path_str, local_datetime);
        let len = 0;
        writeln!(stream, "HTTP/1.1 404 NOT FOUND").unwrap();
        writeln!(stream, "Content-Type: text/html; charset=UTF-8").unwrap();
        writeln!(stream, "Content-Length: {}", len).unwrap();
        writeln!(stream).unwrap();
        return;
    }

    if path.is_dir() {
        let html = get_dir_list(&path);
        writeln!(stream, "HTTP/1.1 200 OK").unwrap();
        writeln!(stream, "Content-Type: text/html; charset=UTF-8").unwrap();
        writeln!(stream, "Content-Length: {}", html.len()).unwrap();
        writeln!(stream).unwrap();
        writeln!(stream, "{}", html).unwrap();
    } else {
        println!("Found: {} - {}", &path_str, local_datetime);

        let mut file = match File::open(&path) {
            Err(why) => {
                panic!("couldn't open {}: {}", path.display(), &why.to_string())
            }
            Ok(file) => file,
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);

        let content_type = get_mime(&path);
        writeln!(stream, "HTTP/1.1 200 OK").unwrap();
        writeln!(stream, "Content-Type: {}; charset=UTF-8", content_type).unwrap();
        writeln!(stream, "Content-Length: {}", len).unwrap();
        writeln!(stream).unwrap();

        // file -> stream
        // ファイルを読み込むための追加のバッファが必要ない点に注意
        io::copy(&mut file, stream).unwrap();
    }
}

#[test]
#[cfg(target_os = "windows")]
fn test_make_path() {
    assert_eq!(
        make_path(".", "sample_file/aaa/bbb.txt"),
        ".\\sample_file\\aaa\\bbb.txt".to_string()
    );

    assert_eq!(
        make_path("C:www\\html\\", "sample_file/aaa/bbb.txt"),
        "C:www\\html\\sample_file\\aaa\\bbb.txt".to_string()
    );

    assert_eq!(
        make_path("C:www\\html\\", "sample_file/aaa/"),
        "C:www\\html\\sample_file\\aaa\\index.html".to_string()
    );
}
