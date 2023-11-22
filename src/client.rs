use crate::models::ClusterInfo;
use reqwest::{self, Error};
use std::net::TcpStream;
use tungstenite::{connect, stream::MaybeTlsStream, WebSocket};
use url::Url;

pub fn get_cluster_info(host: &String) -> Result<ClusterInfo, Error> {
    let url = format!("{}/context", host);
    let body = reqwest::blocking::get(url)?.text()?;
    let cluster_info: ClusterInfo = serde_json::from_str(&body).expect("Failed to parse JSON");
    Ok(cluster_info)
}

pub fn connect_to_host(
    host: &str,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error>> {
    let mut protocol = "ws";
    let mut actual_host = host.to_owned();
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
    let (socket, _) = match connect(url) {
        Ok((socket, response)) => (socket, response),
        Err(e) => {
            eprintln!("Error connecting to WebSocket: {:?}", e);
            return Err(Box::new(e));
        }
    };
    Ok(socket)
}
