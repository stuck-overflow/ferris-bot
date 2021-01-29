use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufRead, BufReader, Error, Read, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};
use std::str;
use tempfile::tempdir;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{IRCMessage, PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;

fn parse_command(msg: PrivmsgMessage) {
    let first_word = msg.message_text.split_whitespace().next();
    let content = msg.message_text.replace(first_word.as_deref().unwrap(), "");
    match first_word {
        Some("!join") => println!("{}: Join requested", msg.sender.login),
        Some("!Pythonsucks") => println!("{}: This must be Lord", msg.sender.login),
        Some("!Stonk") => println!("{}: yOu shOULd Buy AMC sTOnKS", msg.sender.login),
        Some("!C++") => println!("{}: segmentation fault", msg.sender.login),
        Some("!Dave") => println!("{}", include_str!("../assets/dave.txt")),
        Some("!Bazylia") => println!("{}", include_str!("../assets/bazylia.txt")),
        Some("!Zoya") => println!("{}", include_str!("../assets/zoya.txt")),
        Some("!Discord") => println!("https://discord.gg/UyrsFX7N"),
        Some("!code") => save_code_format(&content),
        _ => {}
    }
}

fn temp_code_format(message: &str) {
    println!("{}", message);
    let dir = tempdir().unwrap();
    let mut file_path = dir.path().join("chat_code.rs");
    let path = File::create(&file_path);
    println!("{}", file_path.display());
    let _ = fs::write(&file_path, message).expect("Unable to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg(&file_path);
    tidy.status().expect("not working");
    let content = fs::read_to_string(&file_path).expect("error reading file");
    println!("{}", &content);
}

fn save_code_format(message: &str) {
    let path = "chat_code.rs";
    let _ = File::create(path);
    fs::write(path, message).expect("Unable to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg(path);
    tidy.status().expect("not working");
}

#[tokio::main]
pub async fn main() {
    // default configuration is to join chat as anonymous.
    let config = ClientConfig::default();
    let (mut incoming_messages, client) =
        TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

    // first thing you should do: start consuming incoming messages,
    // otherwise they will back up.
    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => parse_command(msg),
                _ => continue,
            }
        }
    });

    // join a channel
    client.join("stuck_overflow".to_owned());

    // keep the tokio executor alive.
    // If you return instead of waiting the background task will exit.
    join_handle.await.unwrap();
}
