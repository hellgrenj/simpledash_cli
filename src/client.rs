use tungstenite::{connect, stream::MaybeTlsStream, WebSocket};
use crate::models::ClusterInfo;

use url::Url;
use reqwest::{self, Error};
use colored::*;
use std::net::TcpStream;

pub fn get_cluster_info(host: &String) -> Result<ClusterInfo, Error> {
    let url = format!("{}/context", host);
    let body = reqwest::blocking::get(url)?.text()?;
    let cluster_info: ClusterInfo = serde_json::from_str(&body).expect("Failed to parse JSON");
    return Ok(cluster_info);
}

pub fn connect_to_host(
    host: &String,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error>> {
    println!("Connecting to {}", host);
    let mut protocol = "ws";
    let mut actual_host = host.clone();
    if host.split_at(8).0 == "https://" {
        protocol = "wss";
        actual_host = host.replace("https://", "");
    } else if host.split_at(7).0 == "http://" {
        actual_host = host.replace("http://", "");
    }
    let url_str = format!("{}://{}/ws", protocol, actual_host);
    let url = match Url::parse(&url_str) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Error parsing URL: {:?}", e);
            return Err(Box::new(e));
        }
    };
    let (socket, _) = match connect(&url) {
        Ok((socket, response)) => (socket, response),
        Err(e) => {
            eprintln!("Error connecting to WebSocket: {:?}", e);
            return Err(Box::new(e));
        }
    };

    println!(
        "successfully connected to {}",
        url.to_string().magenta().bold().on_black()
    );
    Ok(socket)
}