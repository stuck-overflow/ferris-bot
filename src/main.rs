extern crate serenity;
use serenity::http::Http;
use serenity::model::id::ChannelId;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::str;
use std::sync::Arc;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;
mod discord_commands;

async fn parse_command(msg: PrivmsgMessage, http: &Arc<Http>) {
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
        Some("!nothing") => nothing(&http).await,
        Some("!code") => save_code_format(&http, &content).await,
        _ => {}
    }
}

async fn nothing(http: &Arc<Http>) {
    println!("nothing received");
    let id: u64 = 805839708198404106;
    let _ = ChannelId(id).say(http, "This does nothing").await;
}

async fn send_code_discord(http: &Arc<Http>, code_file: &Path) {
    let code_ex = fs::read_to_string(code_file).expect("nop you nop read file");
    let code_ex = format!("{}{}{}", "```rs\n", code_ex, "```");
    let id: u64 = 805839708198404106;
    let _ = ChannelId(id).say(http, code_ex).await;
}

async fn save_code_format(http: &Arc<Http>, message: &str) {
    let path = "chat_code.rs";
    let mut file_path = File::create(path).unwrap();
    write!(file_path, "{}", message).expect("not able to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg(path);
    tidy.status().expect("not working");
    let path = Path::new(path);
    send_code_discord(http, path).await;
}

#[tokio::main]
pub async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let mut file = File::open(".token").expect("Error loading Discord token");
    let mut token = String::new();
    file.read_to_string(&mut token)
        .expect("Token file not found");
    let token = token.trim();

    let http = Arc::new(Http::new_with_token(&token));

    // default configuration is to join chat as anonymous.
    let config = ClientConfig::default();
    let (mut incoming_messages, client) =
        TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

    let http2 = Arc::clone(&http);
    // first thing you should do: start consuming incoming messages,
    // otherwise they will back up.
    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => parse_command(msg, &http2).await,
                _ => continue,
            }
        }
    });

    // join a channel
    client.join("stuck_overflow".to_owned());

    discord_commands::init_discord_bot(Arc::clone(&http), &token).await;
    // keep the tokio executor alive.
    // If you return instead of waiting the background task will exit.
    join_handle.await.unwrap();
}
