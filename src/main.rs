mod queue_manager;
mod token_storage;

use itertools::join;
use log::{debug, trace, LevelFilter};
use queue_manager::QueueManager;
use queue_manager::QueueManagerJoinError;
use queue_manager::QueueManagerLeaveError;
use serde::Deserialize;
use simple_logger::SimpleLogger;
use std::sync::Mutex;
use std::{fs, str};
use structopt::StructOpt;
use token_storage::CustomTokenStorage;
use twitch_api2::helix::subscriptions::GetBroadcasterSubscriptionsRequest;
use twitch_api2::helix::users::GetUsersRequest;
use twitch_api2::TwitchClient;
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage};
use twitch_irc::message::Badge;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::{ClientConfig, TCPTransport, TwitchIRCClient};
use twitch_oauth2::Scope;

#[derive(Clone, Deserialize)]
struct FerrisBotConfig {
    twitch: TwitchConfig,
    queue_manager: QueueManagerConfig,
}

#[derive(Clone, Deserialize)]
struct TwitchConfig {
    token_filepath: String,
    login_name: String,
    channel_name: String,
    client_id: String,
    secret: String,
}

#[derive(Clone, Deserialize)]
struct QueueManagerConfig {
    capacity: usize,
    queue_storage: String,
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

    let mut token_storage = CustomTokenStorage {
        token_checkpoint_file: config.twitch.token_filepath.clone(),
    };

    // If we have some errors while loading the stored token, e.g. if we never
    // stored one before or it's unparsable, go through the authentication
    // workflow.
    if let Err(_) = token_storage.load_token().await {
        let user_token = twitch_oauth2_auth_flow::auth_flow(
            &config.twitch.client_id,
            &config.twitch.secret,
            Some(vec![
                Scope::ChannelReadSubscriptions,
                Scope::ChatEdit,
                Scope::ChatRead,
            ]),
        );
        token_storage
            .write_twitch_oauth2_user_token(
                &user_token,
                Some(oauth2::ClientSecret::new(config.twitch.secret.clone())),
            )
            .unwrap();
    }

    let irc_config = ClientConfig::new_simple(RefreshingLoginCredentials::new(
        config.twitch.login_name.clone(),
        config.twitch.client_id.clone(),
        config.twitch.secret.clone(),
        token_storage.clone(),
    ));

    let (mut incoming_messages, twitch_irc_client) =
        TwitchIRCClient::<TCPTransport, _>::new(irc_config);

    let context = Context {
        ferris_bot_config: config.clone(),
        queue_manager: Mutex::new(QueueManager::new(
            config.queue_manager.capacity,
            &config.queue_manager.queue_storage,
        )),
        twitch_irc_client,
        token_storage,
    };

    // join a channel
    context
        .twitch_irc_client
        .join(config.twitch.channel_name.to_owned());

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

async fn is_user_subscriber(ctx: &Context, user: &str, badges: &Vec<Badge>) -> bool {
    for b in badges {
        if b.name == "founder" || b.name == "subscriber" {
            return true;
        }
    }
    let client = surf::Client::new();
    let twitch_api_client = TwitchClient::with_client(client);
    let token = &ctx.token_storage;
    let token = token.load_twitch_oauth2_user_token().unwrap();
    debug!("{:?}", token);

    let req = GetUsersRequest::builder()
        .login(vec![ctx.ferris_bot_config.twitch.login_name.clone()])
        .build();
    let req = twitch_api_client.helix.req_get(req, &token).await.unwrap();
    let broadcaster = req.data.get(0).unwrap();
    let req = GetBroadcasterSubscriptionsRequest::builder()
        .broadcaster_id(broadcaster.id.clone())
        .user_id(vec![user.to_owned()])
        .build();
    debug!("{:?}", req);
    let req = tokio::spawn(async move { twitch_api_client.helix.req_get(req, &token).await })
        .await
        .unwrap();
    debug!("{:?}", req);

    match req {
        Ok(r) => r.data.len() != 0,
        Err(_) => false,
    }
}
struct Context {
    ferris_bot_config: FerrisBotConfig,
    twitch_irc_client:
        TwitchIRCClient<TCPTransport, RefreshingLoginCredentials<CustomTokenStorage>>,
    queue_manager: Mutex<QueueManager>,
    token_storage: CustomTokenStorage,
}

#[derive(Debug, PartialEq)]
enum TwitchCommand {
    Join,
    Queue,
    Leave,
    Next,
    Kick,
    ReplyWith(&'static str),
    Broadcast(&'static str),
}

impl TwitchCommand {
    async fn handle(self, msg: PrivmsgMessage, ctx: &Context) {
        match self {
            TwitchCommand::Join => {
                debug!("Join received");

                let user_type = if is_user_subscriber(&ctx, &msg.sender.login, &msg.badges).await {
                    queue_manager::UserType::Subscriber
                } else {
                    queue_manager::UserType::Default
                };
                let result = ctx
                    .queue_manager
                    .lock()
                    .unwrap()
                    .join(&msg.sender.login, user_type);

                let message: &str;
                match result {
                    Err(QueueManagerJoinError::QueueFull) => message = "The Queue is full",
                    Err(QueueManagerJoinError::UserAlreadyInQueue) => {
                        message = "You have already joined the queue"
                    }
                    Ok(()) => message = "Successfully joined the queue",
                }

                ctx.twitch_irc_client
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
                ctx.twitch_irc_client
                    .say(
                        msg.channel_login,
                        format!("@{}: Current queue: {}", msg.sender.login, reply),
                    )
                    .await
                    .unwrap();
            }

            TwitchCommand::ReplyWith(reply) => {
                ctx.twitch_irc_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", msg.sender.login, reply),
                    )
                    .await
                    .unwrap();
            }

            TwitchCommand::Broadcast(message) => {
                ctx.twitch_irc_client
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

                ctx.twitch_irc_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", &msg.sender.login, message),
                    )
                    .await
                    .unwrap();
            }
            TwitchCommand::Next => {
                if &msg.sender.login != &ctx.ferris_bot_config.twitch.channel_name {
                    return;
                }
                let result = ctx.queue_manager.lock().unwrap().next();

                let message = match result {
                    Some(next_user) => format!("@{} is the next user to play!", next_user),
                    None => "There are no users in the queue".to_owned(),
                };

                ctx.twitch_irc_client
                    .say(
                        msg.channel_login,
                        format!("@{}: {}", &msg.sender.login, message),
                    )
                    .await
                    .unwrap();
            }
            TwitchCommand::Kick => {
                if &msg.sender.login != &ctx.ferris_bot_config.twitch.channel_name {
                    return;
                }

                let first_word = &msg.message_text[5..].trim().split(" ").next();
                let message = match first_word {
                    None => "Please specify which user to kick".to_owned(),
                    Some(word) => {
                        let user = word.trim_start_matches("@").to_lowercase();
                        let result = ctx.queue_manager.lock().unwrap().kick(&user);
                        match result {
                            Err(QueueManagerLeaveError::UserNotInQueue) => {
                                format!("User {} is not in queue", user)
                            }
                            Ok(()) => format!("User {} successfully left the Queue", user),
                        }
                    }
                };

                ctx.twitch_irc_client
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
            ("!kick", _) => Some(TwitchCommand::Kick),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
