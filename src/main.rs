extern crate serenity;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serenity::http::Http;
use serenity::model::id::ChannelId;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::str;
use std::sync::Arc;
use structopt::StructOpt;
use twitch_irc::login::LoginCredentials;
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage, UserAccessToken};
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::Transport;
use twitch_irc::TwitchIRCClient;
mod discord_commands;

async fn parse_command<T: Transport, L: LoginCredentials>(
    msg: PrivmsgMessage,
    client: &Arc<TwitchIRCClient<T, L>>,
    http: &Arc<Http>,
    twitch_channel_name: &String,
    discord_channel_id: u64,
) {
    let first_word = msg.message_text.split_whitespace().next();
    let content = msg.message_text.replace(first_word.as_deref().unwrap(), "");
    let first_word = first_word.unwrap().to_lowercase();
    let first_word = Some(first_word.as_str());

    match first_word {
        Some("!join") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("@{}: Join requested", msg.sender.login),
            )
            .await
            .unwrap(),
        Some("!pythonsucks") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("@{}: This must be Lord", msg.sender.login),
            )
            .await
            .unwrap(),
        Some("!stonk") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("@{}: yOu shOULd Buy AMC sTOnKS", msg.sender.login),
            )
            .await
            .unwrap(),
        Some("!c++") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("@{}: segmentation fault", msg.sender.login),
            )
            .await
            .unwrap(),
        Some("!dave") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("{}", include_str!("../assets/dave.txt")),
            )
            .await
            .unwrap(),
        Some("!bazylia") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("{}", include_str!("../assets/bazylia.txt")),
            )
            .await
            .unwrap(),
        Some("!zoya") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("{}", include_str!("../assets/zoya.txt")),
            )
            .await
            .unwrap(),
        Some("!discord") => client
            .say(
                twitch_channel_name.to_owned(),
                format!("https://discord.gg/UyrsFX7N"),
            )
            .await
            .unwrap(),
        Some("!nothing") => nothing(&http, discord_channel_id).await,
        Some("!code") => save_code_format(&http, &content, discord_channel_id).await,
        _ => {}
    }
}

async fn nothing(http: &Arc<Http>, discord_channel_id: u64) {
    println!("nothing received");
    let _ = ChannelId(discord_channel_id)
        .say(http, "This does nothing")
        .await;
}

async fn send_code_discord(http: &Arc<Http>, discord_channel_id: u64, code_file: &Path) {
    let code_ex = fs::read_to_string(code_file).expect("nop you nop read file");
    let code_ex = format!("{}{}{}", "```rs\n", code_ex, "```");
    let _ = ChannelId(discord_channel_id).say(http, code_ex).await;
}

async fn save_code_format(http: &Arc<Http>, message: &str, discord_channel_id: u64) {
    let path = "chat_code.rs";
    let mut file_path = File::create(path).unwrap();
    write!(file_path, "{}", message).expect("not able to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg(path);
    tidy.status().expect("not working");
    let path = Path::new(path);
    send_code_discord(http, discord_channel_id, path).await;
}

#[derive(Debug)]
struct CustomTokenStorage {
    token_checkpoint_file: String,
}

#[async_trait]
impl TokenStorage for CustomTokenStorage {
    type LoadError = std::io::Error; // or some other error
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        println!("load_token called");
        let token = fs::read_to_string(&self.token_checkpoint_file).unwrap();
        let token: UserAccessToken = serde_json::from_str(&token).unwrap();
        Ok(token)
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        println!("update_token called");
        let serialized = serde_json::to_string(&token).unwrap();
        let _ = File::create(&self.token_checkpoint_file);
        fs::write(&self.token_checkpoint_file, serialized)
            .expect("Twitch IRC: Unable to write token to checkpoint file");
        Ok(())
    }
}

#[derive(Deserialize)]
struct FerrisBotConfig {
    twitch: TwitchConfig,
    discord: DiscordConfig,
}

#[derive(Deserialize)]
struct TwitchConfig {
    token_filepath: String,
    login_name: String,
    channel_name: String,
    client_id: String,
    secret: String,
}

#[derive(Deserialize)]
struct DiscordConfig {
    auth_token: String,
    channel_id: u64,
}

#[derive(Deserialize)]
struct FirstToken {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
}

// Command-line arguments for the tool.
#[derive(StructOpt)]
struct Cli {
    /// Twitch credential files.
    #[structopt(short, long, default_value = "ferrisbot.toml")]
    config_file: String,

    /// Generates the curl command to obtain the first token and exits.
    #[structopt(short, long)]
    generate_curl_first_token_request: bool,

    /// Auth code to be used when obtaining first token.
    #[structopt(long, default_value = "")]
    auth_code: String,

    /// Show the authentication URL and exits.
    #[structopt(short, long)]
    show_auth_url: bool,

    /// If present, parse the access token from the file passed as argument.
    #[structopt(long, default_value = "")]
    first_token_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MyUserAccessToken {
    access_token: String,
    refresh_token: String,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

#[tokio::main]
pub async fn main() {
    let args = Cli::from_args();

    let config = fs::read_to_string(args.config_file).unwrap();

    let config: FerrisBotConfig = toml::from_str(&config).unwrap();

    if args.show_auth_url {
        println!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost&response_type=code&scope=chat:read%20chat:edit", config.twitch.client_id);
        std::process::exit(0);
    }

    if args.generate_curl_first_token_request {
        if args.auth_code.is_empty() {
            println!("Please set --auth_code. Aborting.");
            std::process::exit(1);
        }
        println!("curl -X POST 'https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&code={}&grant_type=authorization_code&redirect_uri=http://localhost' > /tmp/firsttoken.json",
            config.twitch.client_id,
            config.twitch.secret,
            args.auth_code);
        std::process::exit(0);
    }

    let mut storage = CustomTokenStorage {
        token_checkpoint_file: config.twitch.token_filepath,
    };
    if !args.first_token_file.is_empty() {
        let first_token = fs::read_to_string(args.first_token_file).unwrap();
        let first_token: FirstToken = serde_json::from_str(&first_token).unwrap();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::seconds(first_token.expires_in);
        let user_access_token = MyUserAccessToken {
            access_token: first_token.access_token,
            refresh_token: first_token.refresh_token,
            created_at: created_at,
            expires_at: Some(expires_at),
        };
        let serialized = serde_json::to_string(&user_access_token).unwrap();
        let user_access_token: UserAccessToken = serde_json::from_str(&serialized).unwrap();
        storage.update_token(&user_access_token).await.unwrap();
    }

    let irc_config = ClientConfig::new_simple(RefreshingLoginCredentials::new(
        config.twitch.login_name,
        config.twitch.client_id,
        config.twitch.secret,
        storage,
    ));
    let (mut incoming_messages, client) = TwitchIRCClient::<
        TCPTransport,
        RefreshingLoginCredentials<CustomTokenStorage>,
    >::new(irc_config);
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    // Discord credentials.

    let http = Arc::new(Http::new_with_token(&config.discord.auth_token));

    let http2 = Arc::clone(&http);

    let discord_channel_id_clone = config.discord.channel_id.clone();
    let twitch_channel_name_clone = config.twitch.channel_name.clone();
    // first thing you should do: start consuming incoming messages,
    // otherwise they will back up.
    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => {
                    parse_command(
                        msg,
                        &client_clone,
                        &http2,
                        &twitch_channel_name_clone,
                        discord_channel_id_clone,
                    )
                    .await
                }
                _ => continue,
            }
        }
    });

    // join a channel
    client.join(config.twitch.channel_name.to_owned());
    client
        .say(
            config.twitch.channel_name.to_owned(),
            "Hello! I am the Stuck-Bot, How may I unstick you?".to_owned(),
        )
        .await
        .unwrap();

    discord_commands::init_discord_bot(Arc::clone(&http), &config.discord.auth_token).await;
    // keep the tokio executor alive.
    // If you return instead of waiting the background task will exit.
    join_handle.await.unwrap();
}
