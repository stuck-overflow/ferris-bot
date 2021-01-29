
use std::fs;
use std::str;
use std::fs::File;
use std::io::prelude::*;
use std::process::{Command, Stdio};use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;
use twitch_irc::message::{IRCMessage,PrivmsgMessage, ServerMessage};

fn parse_command(msg: PrivmsgMessage) {
    let first_word = msg.message_text.split_whitespace().next();
    let content = msg.message_text.replace(first_word.as_deref().unwrap(), "");
    match first_word {
        Some("!join") => println!("{}: Join requested", msg.sender.login),
        Some("!Pythonsucks")=> println!("{}: This must be Lord", msg.sender.login),
        Some("!Stonk")=> println!("{}: yOu shOULd Buy AMC sTOnKS", msg.sender.login),
        Some("!C++")=> println!("{}: segmentation fault", msg.sender.login),
        Some("!Dave")=> println!("{}", include_str!("../assets/dave.txt")),
        Some("!Bazylia") => println!("{}", include_str!("../assets/bazylia.txt")),    
        Some("!Zoya") => println!("{}", include_str!("../assets/zoya.txt")),
        Some("!Discord") => println!("https://discord.gg/UyrsFX7N"),
        Some("!code") =>  format_code(&content),
        _ => {},
    }
}


// Takes code after !code and writes into scrap.rs then formats with fmt - requires rustfmt installed (apt-get)
fn format_code(message:&str) {
    let path = "scrap.rs";
    let _ = File::create(path);
    fs::write(path, message).expect("Unable to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg("scrap.rs");
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