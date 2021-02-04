extern crate serenity;

use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::process::Command;
use std::str;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;

struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "?ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "I'M ALIVE!!!!") {
                println!("Error giving message: {:?}", why)
            }
        }
    }
    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is ready", ready.user.name);
    }
}

fn discord() {
    let mut file = File::open(".token").expect("Error loading Discord token");
    let mut token = String::new();
    file.read_to_string(&mut token)
        .expect("Token file not found");
    let mut client = Client::new(&token, Handler).expect("Error creating client");
    if let Err(msg) = client.start() {
        println!("Error: {:?}", msg);
    }
}

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
    discord();
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
