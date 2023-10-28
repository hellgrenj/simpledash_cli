use colored::*;

use crate::models::Settings;

pub fn clear_screen() {
    print!("{}[2J", 27 as char); // clear screen
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char); // position cursor at row 1, col 1
    println!("")
}
pub fn make_link(url: String, anchor_text: String) -> String {
    return format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", url, anchor_text);
}
pub fn parse_args() -> Settings {
    let mut host = "".to_string();
    let args: Vec<String> = std::env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--help" {
            println!("{}", "Usage: sc -h <host>".yellow().bold());
            std::process::exit(0);
        }
        if arg == "--version" {
            println!("{}", "v0.2.1".yellow().bold());
            std::process::exit(0);
        }
        if arg == "-h" {
            if i + 1 < args.len() {
                host = args[i + 1].clone();
            } else {
                eprintln!("{}", "Error: -h requires a host argument".red().bold());
                std::process::exit(1);
            }
        }
    }
    if host == "" {
        eprintln!(
            "{}",
            "You have to provide a host with -h <host>".red().bold()
        );
        std::process::exit(1);
    }
    if host.split_at(7).0 != "http://" && host.split_at(8).0 != "https://" {
        eprintln!(
            "{}",
            "Error: host must start with http:// or https://"
                .red()
                .bold()
        );
        std::process::exit(1);
    }
    return Settings { host };
}
