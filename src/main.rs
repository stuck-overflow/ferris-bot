use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;

fn parse_command(msg: PrivmsgMessage) {
    let first_word = msg.message_text.split_whitespace().next();
    match first_word {
        Some("!join") => println!("{}: Join requested", msg.sender.login),
        Some("!Pythonsucks") => println!("{}: This must be Lord", msg.sender.login),
        Some("!Stonk") => println!("{}: yOu shOULd Buy AMC sTOnKS", msg.sender.login),
        Some("!C++") => println!("{}: segmentation fault", msg.sender.login),
        Some("!Dave") => println!("{}", include_str!("../assets/dave.txt")),
        Some("!Bazylia") => println!("{}", include_str!("../assets/bazylia.txt")),
        Some("!Zoya") => println!("{}", include_str!("../assets/zoya.txt")),
        Some("!Discord") => println!("https://discord.gg/UyrsFX7N"),
        _ => {}
    }
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
