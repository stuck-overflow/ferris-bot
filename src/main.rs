mod queue_manager;
mod twitch_auth;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use itertools::join;
use log::{debug, trace, LevelFilter};
use queue_manager::QueueManager;
use queue_manager::QueueManagerJoinError;
use queue_manager::QueueManagerLeaveError;
use serde::Deserialize;
use simple_logger::SimpleLogger;
use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{fs, io, str};
use structopt::StructOpt;
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage, UserAccessToken};
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::{ClientConfig, TCPTransport, TwitchIRCClient};

#[derive(Debug)]
struct CustomTokenStorage {
    token_checkpoint_file: String,
}

#[async_trait]
impl TokenStorage for CustomTokenStorage {
    type LoadError = std::io::Error; // or some other error
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        debug!("load_token called");
        let token = fs::read_to_string(&self.token_checkpoint_file);
        if let Err(e) = token {
            return Err(e);
        }
        let token: Result<UserAccessToken, _> = serde_json::from_str(&(token.unwrap()));
        if let Err(_) = token {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to deserialize token",
            ));
        }
        Ok(token.unwrap())
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        debug!("update_token called");
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
}

#[derive(Deserialize)]
struct TwitchConfig {
    token_filepath: String,
    login_name: String,
    channel_name: String,
    client_id: String,
    secret: String,
}

// Command-line arguments for the tool.
#[derive(StructOpt)]
struct Cli {
    /// Log level
    #[structopt(short, long, case_insensitive = true, default_value = "INFO")]
    log_level: LevelFilter,

    /// Twitch credential files.
    #[structopt(short, long, default_value = "ferrisbot.toml")]
    config_file: String,
}

#[tokio::main]
pub async fn main() {
    let args = Cli::from_args();
    SimpleLogger::new()
        .with_level(args.log_level)
        .init()
        .unwrap();

    let config = fs::read_to_string(args.config_file).unwrap();
    let config: FerrisBotConfig = toml::from_str(&config).unwrap();

    let mut storage = CustomTokenStorage {
        token_checkpoint_file: config.twitch.token_filepath.clone(),
    };

    // If we have some errors while loading the stored token, e.g. if we never
    // stored one before or it's unparsable, go through the authentication
    // workflow.
    if let Err(_) = storage.load_token().await {
        let first_token = twitch_auth::auth_flow(&config.twitch.client_id, &config.twitch.secret);

        let created_at = Utc::now();
        let expires_at = created_at + Duration::seconds(first_token.expires_in);
        let user_access_token = UserAccessToken {
            access_token: first_token.access_token,
            refresh_token: first_token.refresh_token,
            created_at,
            expires_at: Some(expires_at),
        };
        storage.update_token(&user_access_token).await.unwrap();
    }

    let irc_config = ClientConfig::new_simple(RefreshingLoginCredentials::new(
        config.twitch.login_name.clone(),
        config.twitch.client_id.clone(),
        config.twitch.secret.clone(),
        storage,
    ));

    let (mut incoming_messages, twitch_client) =
        TwitchIRCClient::<TCPTransport, _>::new(irc_config);

    let context = Context {
        queue_manager: Arc::new(Mutex::new(QueueManager::new(3))),
        twitch_client,
    };

    // join a channel
    context
        .twitch_client
        .join(config.twitch.channel_name.to_owned());

    context
        .twitch_client
        .say(
            config.twitch.channel_name.to_owned(),
            "Hello! I am the Stuck-Bot, How may I unstick you?".to_owned(),
        )
        .await
        .unwrap();

    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            trace!("{:?}", message);
            match message {
                ServerMessage::Privmsg(msg) => {
                    if let Some(cmd) = TwitchCommand::parse_msg(&msg) {
                        cmd.handle(msg, &context).await;
                    }
                }
                _ => continue,
            }
        }
    });

    // keep the tokio executor alive.
    // If you return instead of waiting the background task will exit.
    join_handle.await.unwrap();
}

struct Context {
    twitch_client: TwitchIRCClient<TCPTransport, RefreshingLoginCredentials<CustomTokenStorage>>,
    queue_manager: Arc<Mutex<QueueManager>>,
}

#[derive(Debug, PartialEq)]
enum TwitchCommand {
    Join,
    Queue,
    Leave,
    Next,
    ReplyWith(&'static str),
    Broadcast(&'static str),
}

impl TwitchCommand {
    async fn handle(self, msg: PrivmsgMessage, ctx: &Context) {
        match self {
            TwitchCommand::Join => {
                // TODO check if user is subscriber
                let result = ctx
                    .queue_manager
                    .lock()
                    .unwrap()
                    .join(&msg.sender.login, queue_manager::UserType::Default);

                let message: &str;
                match result {
                    Err(QueueManagerJoinError::QueueFull) => message = "The Queue is full",
                    Err(QueueManagerJoinError::UserAlreadyInQueue) => {
                        message = "You have already joined the queue"
                    }
                    Ok(()) => message = "Successfully joined the queue",
                }

                ctx.twitch_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", &msg.sender.login, message),
                    )
                    .await
                    .unwrap();
            }
            TwitchCommand::Queue => {
                let reply = {
                    let queue_manager = ctx.queue_manager.lock().unwrap();
                    join(queue_manager.queue(), ", ")
                };
                ctx.twitch_client
                    .say(
                        msg.channel_login,
                        format!("@{}: Current queue: {}", msg.sender.login, reply),
                    )
                    .await
                    .unwrap();
            }

            TwitchCommand::ReplyWith(reply) => {
                ctx.twitch_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", msg.sender.login, reply),
                    )
                    .await
                    .unwrap();
            }

            TwitchCommand::Broadcast(message) => {
                ctx.twitch_client
                    .say(msg.channel_login, message.to_owned())
                    .await
                    .unwrap();
            }

            TwitchCommand::Leave => {
                let result = ctx.queue_manager.lock().unwrap().leave(&msg.sender.login);

                let message: &str;
                match result {
                    Err(QueueManagerLeaveError::UserNotInQueue) => message = "User is not in queue",
                    Ok(()) => message = "Successfully left the Queue",
                }

                ctx.twitch_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", &msg.sender.login, message),
                    )
                    .await
                    .unwrap();
            }
            TwitchCommand::Next => {
                // TODO check if &msg.sender.login has enough powers.
                let result = ctx.queue_manager.lock().unwrap().next();

                let message: String;
                match result {
                    Some(next_user) => {
                        message = format!("@{} is the next user to play!", next_user)
                    }
                    None => message = "There are no users in the queue".to_owned(),
                }

                ctx.twitch_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", &msg.sender.login, message),
                    )
                    .await
                    .unwrap();
            }
        }
    }

    fn parse_msg(msg: &PrivmsgMessage) -> Option<TwitchCommand> {
        if !msg.message_text.starts_with('!') {
            return None;
        }

        let parts: Vec<&str> = msg.message_text.split_whitespace().collect();
        let (cmd, args) = parts.split_first()?;

        match (cmd.to_lowercase().as_str(), args) {
            ("!join", _) => Some(TwitchCommand::Join),
            ("!leave", _) => Some(TwitchCommand::Leave),
            ("!queue", _) => Some(TwitchCommand::Queue),
            ("!next", _) => Some(TwitchCommand::Next),
            ("!pythonsucks", _) => Some(TwitchCommand::ReplyWith("This must be Lord")),
            ("!stonk", _) => Some(TwitchCommand::ReplyWith("yOu shOULd Buy AMC sTOnKS")),
            ("!c++", _) => Some(TwitchCommand::ReplyWith("segmentation fault")),
            ("!dave", _) => Some(TwitchCommand::Broadcast(include_str!("../assets/dave.txt"))),
            ("!bazylia", _) => Some(TwitchCommand::Broadcast(include_str!(
                "../assets/bazylia.txt"
            ))),
            ("!zoya", _) => Some(TwitchCommand::Broadcast(include_str!("../assets/zoya.txt"))),
            ("!discord", _) => Some(TwitchCommand::Broadcast("https://discord.gg/UyrsFX7N")),
            ("!nothing", _) => Some(TwitchCommand::ReplyWith("this commands does nothing!")),
            _ => None,
        }
    }
}

fn format_snippet(snippet: &str) -> Result<String, io::Error> {
    let mut rustfmt = Command::new("rustfmt")
        .args(&["--config", "newline_style=Unix"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let input = rustfmt.stdin.as_mut().unwrap();
    input.write_all(snippet.as_bytes())?;

    let output = rustfmt.wait_with_output()?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_commands() {
        assert!(TwitchCommand::parse_msg(&test_msg("regular message text")).is_none());
        assert_eq!(
            TwitchCommand::parse_msg(&test_msg("!join")),
            Some(TwitchCommand::Join)
        );

        // commands should be case-insensitive with their arguments left untouched
        assert_eq!(
            TwitchCommand::parse_msg(&test_msg("!sToNk")),
            Some(TwitchCommand::ReplyWith("yOu shOULd Buy AMC sTOnKS"))
        );
    }

    #[test]
    fn formatting_snippets() {
        // converting to Option here because std::io::Error doesn't impl PartialEq
        assert_eq!(
            format_snippet(r#"fn main() { println!("hello world"); }"#)
                .as_deref()
                .ok(),
            Some("fn main() {\n    println!(\"hello world\");\n}\n")
        );

        assert!(format_snippet(r#"totally not rust code"#).is_err());
    }

    fn test_msg(message_text: &str) -> PrivmsgMessage {
        use twitch_irc::message::{IRCMessage, IRCTags, TwitchUserBasics};

        PrivmsgMessage {
            channel_login: "channel_login".to_owned(),
            channel_id: "channel_id".to_owned(),
            message_text: message_text.to_owned(),
            is_action: false,
            sender: TwitchUserBasics {
                id: "12345678".to_owned(),
                login: "login".to_owned(),
                name: "name".to_owned(),
            },
            badge_info: vec![],
            badges: vec![],
            bits: None,
            name_color: None,
            emotes: vec![],
            server_timestamp: Utc::now(),
            message_id: "1094e782-a8fc-4d95-a589-ad53e7c13d25".to_owned(),
            source: IRCMessage {
                tags: IRCTags::default(),
                prefix: None,
                command: String::new(),
                params: vec![],
            },
        }
    }
}
