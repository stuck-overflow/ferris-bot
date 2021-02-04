extern crate serenity;
use std::fs;
use std::fs::File;
use std::process::Command;
use std::str;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;
mod discord_commands;

fn parse_command(msg: PrivmsgMessage) {
    let first_word = msg.message_text.split_whitespace().next();
    let content = msg.message_text.replace(first_word.as_deref().unwrap(), "");
    let first_word = first_word.unwrap().to_lowercase();
    let first_word = Some(first_word.as_str());

    match first_word {
        Some("!join") => println!("{}: Join requested", msg.sender.login),
        Some("!pythonsucks") => println!("{}: This must be Lord", msg.sender.login),
        Some("!stonk") => println!("{}: yOu shOULd Buy AMC sTOnKS", msg.sender.login),
        Some("!c++") => println!("{}: segmentation fault", msg.sender.login),
        Some("!dave") => println!("{}", include_str!("../assets/dave.txt")),
        Some("!bazylia") => println!("{}", include_str!("../assets/bazylia.txt")),
        Some("!zoya") => println!("{}", include_str!("../assets/zoya.txt")),
        Some("!discord") => println!("https://discord.gg/UyrsFX7N"),
        Some("!code") => save_code_format(&content),
        _ => {}
    }
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
    discord_commands::init_discord_bot().await;
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
