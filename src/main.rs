mod queue_manager;
mod token_storage;
mod word_stonks;

use giphy_api::Giphy;
use itertools::join;
use log::{debug, trace, LevelFilter};
use obws::requests::{SceneItemRender, SourceSettings};
use obws::Client;
use queue_manager::{QueueManager, QueueManagerJoinError, QueueManagerLeaveError};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use simple_logger::SimpleLogger;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use std::{fs, str};
use structopt::StructOpt;
use token_storage::CustomTokenStorage;
use tokio_compat_02::FutureExt;
use twitch_api2::helix::subscriptions::GetBroadcasterSubscriptionsRequest;
use twitch_api2::helix::users::GetUsersRequest;
use twitch_api2::twitch_oauth2::Scope;
use twitch_api2::TwitchClient;
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage};
use twitch_irc::message::{Badge, PrivmsgMessage, ServerMessage};
use twitch_irc::{ClientConfig, TCPTransport, TwitchIRCClient};
use word_stonks::{GuessResult, WordStonksGame};

#[derive(Clone, Deserialize)]
struct FerrisBotConfig {
    twitch: TwitchConfig,
    giphy: Option<GiphyConfig>,
    queue_manager: Option<QueueManagerConfig>,
    obs: Option<ObsConfig>,
}

#[derive(Clone, Deserialize)]
struct ObsConfig {
    host: String,
    port: u16,
    password: String,
    gif_source: Option<String>,
    scene_1: Option<String>,
    scene_2: Option<String>,
    alan_box_sourceitem: Option<String>,
    audio_folder: Option<PathBuf>,
    sounds: Option<Vec<String>>,
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
struct GiphyConfig {
    api_key: String,
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

    let config = match fs::read_to_string(&args.config_file) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Error opening the configuration file {}: {}",
                args.config_file, e
            );
            eprintln!("Create the file or use the --config_file flag to specify an alternative file location");
            return;
        }
    };

    let config: FerrisBotConfig = match toml::from_str(&config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Error parsing configuration file {}: {}",
                args.config_file, e
            );
            return;
        }
    };

    let mut token_storage = CustomTokenStorage {
        token_checkpoint_file: config.twitch.token_filepath.clone(),
    };

    // If we have some errors while loading the stored token, e.g. if we never
    // stored one before or it's unparsable, go through the authentication
    // workflow.
    if let Err(_) = token_storage.load_token().await {
        let user_token = match twitch_oauth2_auth_flow::auth_flow(
            &config.twitch.client_id,
            &config.twitch.secret,
            Some(vec![
                Scope::ChannelReadSubscriptions,
                Scope::ChatEdit,
                Scope::ChatRead,
            ]),
            "http://localhost:10666/twitch/token",
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error during the authentication flow: {}", e);
                return;
            }
        };
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

    let obs_client = match &config.obs {
        None => None,
        Some(obs_cfg) => {
            let client = Client::connect(&obs_cfg.host, obs_cfg.port).await.unwrap();
            client.login(Some(&obs_cfg.password)).await.unwrap();
            Some(client)
        }
    };

    let queue_manager = match &config.queue_manager {
        None => None,
        Some(cfg) => Some(Mutex::new(QueueManager::new(
            cfg.capacity,
            &cfg.queue_storage,
        ))),
    };

    let mut context = Context {
        ferris_bot_config: config.clone(),
        twitch_irc_client,
        queue_manager,
        token_storage,
        word_stonks_game: None,
        obs_client,
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
                        cmd.handle(msg, &mut context).await;
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
    queue_manager: Option<Mutex<QueueManager>>,
    token_storage: CustomTokenStorage,
    word_stonks_game: Option<WordStonksGame>,
    obs_client: Option<Client>,
}

#[derive(Debug, PartialEq)]
enum TwitchCommand {
    Join,
    Queue,
    Leave,
    Next,
    Kick,
    Switch,
    Gif,
    Sound,
    AlanBox,
    ReplyWith(&'static str),
    Broadcast(&'static str),
    WordGuess,
    WordStonks,
}

impl TwitchCommand {
    async fn handle(self, msg: PrivmsgMessage, ctx: &mut Context) {
        match self {
            TwitchCommand::Join => {
                let queue_manager = match &ctx.queue_manager {
                    None => return,
                    Some(q) => q,
                };

                debug!("Join received");

                let user_type = if is_user_subscriber(&ctx, &msg.sender.login, &msg.badges).await {
                    queue_manager::UserType::Subscriber
                } else {
                    queue_manager::UserType::Default
                };
                let result = queue_manager
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
                let queue_manager = match &ctx.queue_manager {
                    None => return,
                    Some(q) => q,
                };

                let reply = {
                    let queue_manager = queue_manager.lock().unwrap();
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
                let queue_manager = match &ctx.queue_manager {
                    None => return,
                    Some(q) => q,
                };
                let result = queue_manager.lock().unwrap().leave(&msg.sender.login);

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
                let queue_manager = match &ctx.queue_manager {
                    None => return,
                    Some(q) => q,
                };
                if &msg.sender.login != &ctx.ferris_bot_config.twitch.channel_name {
                    return;
                }
                let result = queue_manager.lock().unwrap().next();

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
            TwitchCommand::AlanBox => {
                let obs_client = match &ctx.obs_client {
                    None => return,
                    Some(c) => c,
                };
                let alan_box_sourceitem = match &ctx
                    .ferris_bot_config
                    .obs
                    .as_ref()
                    .unwrap()
                    .alan_box_sourceitem
                {
                    None => return,
                    Some(g) => g,
                };
                let alan_text = &msg.message_text[8..].trim();
                if alan_text.len() == 0 {
                    ctx.twitch_irc_client
                        .say(
                            msg.channel_login,
                            format!(
                                "@{}: {}",
                                &msg.sender.login,
                                "Leave a message to Alan!".to_owned()
                            ),
                        )
                        .await
                        .unwrap();
                    return;
                }

                // Create new lines if input by user.
                let re = Regex::new(r#"\\n"#).unwrap();
                let alan_text = re.replace_all(alan_text, "\n");

                // Parse the lines and wrap every line at max_cols characters.
                let mut output = String::new();
                let mut current_line;
                let max_cols = 50;
                for line in alan_text.split("\n") {
                    if line.len() < max_cols {
                        if output.len() > 0 {
                            output.push_str("\n");
                        }
                        output.push_str(&line);
                        continue;
                    }
                    current_line = String::new();
                    for s in line.split(" ") {
                        if (current_line.len() == 0) && (s.len() > max_cols) {
                            if output.len() > 0 {
                                output.push_str("\n");
                            }
                            output.push_str(&s);
                            continue;
                        }
                        if (current_line.len() + 1 + s.len()) > max_cols {
                            if output.len() > 0 {
                                output.push_str("\n");
                            }
                            output.push_str(&current_line);
                            current_line = s.to_string();
                        } else {
                            current_line.push_str(" ");
                            current_line.push_str(&s);
                        }
                    }
                    if output.len() > 0 {
                        output.push_str("\n");
                    }
                    output.push_str(&current_line);
                }
                // Get a list of available scenes and print them out.
                let sources = obs_client.sources();
                let alan_box = sources
                    .get_source_settings::<serde_json::Value>(alan_box_sourceitem, None)
                    .await;
                if let Err(e) = alan_box {
                    eprintln!("Can't find OBS source {} for : {}", &alan_box_sourceitem, e);
                    return;
                }
                let settings = alan_box.unwrap();

                if let Err(e) = sources
                    .set_source_settings::<serde_json::Value>(SourceSettings {
                        source_name: &settings.source_name,
                        source_type: Some(&settings.source_type),
                        source_settings: &json!({ "text": output }),
                    })
                    .await
                {
                    println!("Error while setting source settings: {}", e);
                }
            }
            TwitchCommand::Gif => {
                let obs_client = match &ctx.obs_client {
                    None => return,
                    Some(c) => c,
                };
                let gif_source = match &ctx.ferris_bot_config.obs.as_ref().unwrap().gif_source {
                    None => return,
                    Some(g) => g,
                };

                let api_key = match &ctx.ferris_bot_config.giphy {
                    None => return,
                    Some(giphy_config) => &giphy_config.api_key,
                };
                let query = &msg.message_text[4..].trim();
                if query.len() == 0 {
                    ctx.twitch_irc_client
                        .say(
                            msg.channel_login,
                            format!(
                                "@{}: {}",
                                &msg.sender.login,
                                "Enter a word to search for a gif".to_owned()
                            ),
                        )
                        .await
                        .unwrap();
                    return;
                }

                let giphy_client = Giphy::new(api_key);
                let gif = giphy_client.search_gifs(query, 5, "pg-13").compat().await;
                if let Err(e) = gif {
                    println!("error {}", e);
                    return;
                }
                let gif = gif.unwrap();
                if gif.len() == 0 {
                    ctx.twitch_irc_client
                        .say(
                            msg.channel_login,
                            format!("@{}: No results for query: {}", &msg.sender.login, query),
                        )
                        .await
                        .unwrap();
                    return;
                }
                let gif = &gif[0].url;

                // Get a list of available scenes and print them out.
                let sources = obs_client.sources();
                let gifitem = sources
                    .get_source_settings::<serde_json::Value>(gif_source, None)
                    .await;
                if let Err(e) = gifitem {
                    eprintln!(
                        "Can't find OBS source {} for Gif display: {}",
                        &gif_source, e
                    );
                    return;
                }
                dbg!(&gifitem);
                //https://media.giphy.com/media/g7GKcSzwQfugw/giphy.gif
                //https://giphy.com/gifs/rick-roll-g7GKcSzwQfugw
                //https://giphy.com/gifs/g7GKcSzwQfugw
                let settings = gifitem.unwrap();

                let media = gif.rsplit("/").next().unwrap();
                let media = media.rsplit("-").next().unwrap();
                let media_url = format!("https://i.giphy.com/media/{}/giphy.webp", media);
                if let Err(e) = sources
                    .set_source_settings::<serde_json::Value>(SourceSettings {
                        source_name: &settings.source_name,
                        source_type: Some(&settings.source_type),
                        source_settings: &json!({ "url": media_url }),
                    })
                    .await
                {
                    println!("Error while setting source settings: {}", e);
                }
                let scene = obs_client.scenes().get_current_scene().await.unwrap();
                let gif_bot = scene.sources.iter().find(|item| &item.name == gif_source);
                if let None = gif_bot {
                    return;
                }

                let scene_item_render = SceneItemRender {
                    scene_name: None,
                    source: gif_source,
                    item: None,
                    render: true,
                };
                if let Err(_) = obs_client
                    .scene_items()
                    .set_scene_item_render(scene_item_render)
                    .await
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;

                let sources = obs_client.sources();
                let gifitem = sources
                    .get_source_settings::<serde_json::Value>(gif_source, None)
                    .await;
                if let Err(e) = gifitem {
                    eprintln!(
                        "Can't find OBS source {} for Gif display: {}",
                        &gif_source, e
                    );
                    return;
                }

                let source_settings = gifitem.unwrap().source_settings;

                let url = source_settings.get("url");
                if let None = url {
                    eprintln!("No url attribute found: {:?}", url);
                    return;
                }
                let url = url.unwrap();
                if url.is_string() {
                    if url.as_str().unwrap() != media_url {
                        return;
                    }
                }
                let scene_item_render = SceneItemRender {
                    scene_name: None,
                    source: &gif_source,
                    item: None,
                    render: false,
                };
                if let Err(_) = obs_client
                    .scene_items()
                    .set_scene_item_render(scene_item_render)
                    .await
                {
                    return;
                }
            }

            TwitchCommand::Switch => {
                let obs_client = match &ctx.obs_client {
                    None => return,
                    Some(c) => c,
                };
                let obs = &ctx.ferris_bot_config.obs.as_ref().unwrap();
                let (scene_1, scene_2) = match obs.scene_1.as_ref().zip(obs.scene_2.as_ref()) {
                    None => return,
                    Some(pair) => pair,
                };

                // Get a list of available scenes and print them out.
                let scene = obs_client.scenes().get_current_scene().await.unwrap();

                let next_scene;
                if &scene.name == scene_1 {
                    next_scene = scene_2;
                } else if &scene.name == scene_2 {
                    next_scene = scene_1;
                } else {
                    return;
                }

                let set_scene = obs_client.scenes().set_current_scene(next_scene).await;
                if let Err(e) = set_scene {
                    eprintln!("Error while changing scene: {}", e);
                    return;
                }
                ctx.twitch_irc_client
                    .say(
                        msg.channel_login,
                        format!("Screen switched to {}", next_scene),
                    )
                    .await
                    .unwrap();
            }

            TwitchCommand::Sound => {
                let obs_client = match &ctx.obs_client {
                    None => return,
                    Some(c) => c,
                };

                let obs = &ctx.ferris_bot_config.obs.as_ref().unwrap();
                let directory = match &obs.audio_folder {
                    None => return,
                    Some(d) => d,
                };
                let sounds = match &obs.sounds {
                    None => return,
                    Some(s) => {
                        if s.is_empty() {
                            return;
                        }
                        s
                    }
                };

                let query = &msg.message_text[6..].trim();
                if query.is_empty() {
                    ctx.twitch_irc_client
                        .say(
                            msg.channel_login,
                            format!("@{}: {}", &msg.sender.login, sounds.join(", ").to_owned()),
                        )
                        .await
                        .unwrap();
                    return;
                }

                dbg!(&directory);
                let sound = query.to_lowercase();
                let sound = match Path::new(&sound).file_name() {
                    None => return,
                    Some(s) => s,
                };
                dbg!(&sounds);
                let sound_path = Path::new(directory).join(sound).with_extension("mp3");
                dbg!(&sound_path);

                // Get a list of available scenes and print them out.
                //let scene = client.scenes().get_current_scene().await.unwrap();
                let sources = obs_client.sources();
                let audio_source = sources
                    .get_source_settings::<serde_json::Value>("Audio_Source", None)
                    .await;
                if let Err(e) = audio_source {
                    println!("Can't find Audio_Source source: {}", e);
                    return;
                }
                dbg!(&audio_source);
                let settings = audio_source.unwrap();
                let sound_path = sound_path.to_str().unwrap();
                if let Err(e) = sources
                    .set_source_settings::<serde_json::Value>(SourceSettings {
                        source_name: &settings.source_name,
                        source_type: Some(&settings.source_type),
                        source_settings: &json!({ "local_file": sound_path }),
                    })
                    .await
                {
                    println!("Error while setting source settings: {}", e);
                }
                let scene = obs_client.scenes().get_current_scene().await.unwrap();

                let audio_source = scene
                    .sources
                    .iter()
                    .find(|item| item.name == "Audio_Source");
                if let None = audio_source {
                    return;
                }

                let scene_item_render = SceneItemRender {
                    scene_name: None,
                    source: "Audio_Source",
                    item: None,
                    render: true,
                };
                if let Err(_) = obs_client
                    .scene_items()
                    .set_scene_item_render(scene_item_render)
                    .await
                {
                    return;
                }
            }
            TwitchCommand::Kick => {
                let queue_manager = match &ctx.queue_manager {
                    None => return,
                    Some(q) => q,
                };
                if &msg.sender.login != &ctx.ferris_bot_config.twitch.channel_name {
                    return;
                }
                let first_word = &msg.message_text[5..].trim().split(" ").next();
                let message = match first_word {
                    None => "Please specify which user to kick".to_owned(),
                    Some(word) => {
                        let user = word.trim_start_matches("@").to_lowercase();
                        let result = queue_manager.lock().unwrap().kick(&user);
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
            TwitchCommand::WordStonks => {
                let message = match &ctx.word_stonks_game {
                    None => {
                        let game = WordStonksGame::new(include_str!("../assets/words.txt"));
                        let interval = game.current_word_interval();
                        let message = format!(
                            "@{} wants to play WordStonks! Guess the hidden word with !wordguess <your_guess> . The hidden word is between {} and {}",
                            &msg.sender.login, interval.lower_bound, interval.upper_bound);
                        ctx.word_stonks_game = Some(game);
                        message
                    }
                    Some(game) => {
                        let interval = game.current_word_interval();
                        format!("@{} WordStonks game is in progress! Guess the hidden word with !wordguess <your_guess> . The current hidden word is between {} and {}",
                                &msg.sender.login, interval.lower_bound, interval.upper_bound)
                    }
                };
                ctx.twitch_irc_client
                    .say(msg.channel_login, message)
                    .await
                    .unwrap();
            }
            TwitchCommand::WordGuess => {
                let message = match &mut ctx.word_stonks_game {
                    None => {
                        return;
                        // format!("@{}: No WordStonks game currently active! Start a game by typing !wordstonks",
                        //         &msg.sender.login)
                    }
                    Some(game) => {
                        let command = &msg.message_text.trim().split(" ").next().unwrap();
                        let first_word =
                            &msg.message_text[command.len()..].trim().split(" ").next();
                        let message = match first_word {
                            None => return,
                            Some(word) => match game.guess(word) {
                                GuessResult::Correct => {
                                    ctx.word_stonks_game = None;
                                    format!("Congratulations! The correct word was \"{}\"", word)
                                }
                                GuessResult::Incorrect(interval) => {
                                    format!(
                                        "\"{}\" : \"{}\", Last guess: {} ({})",
                                        interval.lower_bound, interval.upper_bound, *word, game.hamming_distance(String::from(*word))
                                    )
                                }
                                _ => return,
                                // GuessResult::InvalidWord => {
                                //     format!("The word \"{}\" is not in my vocabulary", word)
                                // }
                                // GuessResult::OutOfRange => {
                                //     let interval = game.current_word_interval();
                                //     format!(
                                //         "The word \"{}\" is not between \"{}\" and \"{}\"",
                                //         word, interval.lower_bound, interval.upper_bound
                                //     )
                                // }
                                // GuessResult::GameOver(_) => String::from("The game is over"),
                            },
                        };
                        message
                        // format!("@{}:\n{}", &msg.sender.login, message)
                    }
                };
                // print to alanbox
                let obs_client = match &ctx.obs_client {
                    None => return,
                    Some(c) => c,
                };
                let stonks_box = match &ctx
                    .ferris_bot_config
                    .obs
                    .as_ref()
                    .unwrap()
                    .alan_box_sourceitem
                {
                    None => return,
                    Some(g) => g,
                };
                let sources = obs_client.sources();

                let alan_box = sources
                    .get_source_settings::<serde_json::Value>(stonks_box, None)
                    .await;
                if let Err(e) = alan_box {
                    eprintln!("Can't find OBS source {} for : {}", stonks_box, e);
                    return;
                }
                let settings = alan_box.unwrap();

                if let Err(e) = sources
                    .set_source_settings::<serde_json::Value>(SourceSettings {
                        source_name: &settings.source_name,
                        source_type: Some(&settings.source_type),
                        source_settings: &json!({ "text": message }),
                    })
                    .await
                {
                    println!("Error while setting source settings: {}", e);
                }
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
            ("!discord", _) => Some(TwitchCommand::Broadcast("https://discord.gg/RhpnmgWQyu")),
            ("!nothing", _) => Some(TwitchCommand::ReplyWith("this commands does nothing!")),
            ("!alanbox", _) => Some(TwitchCommand::AlanBox),
            ("!switchscreen", _) => Some(TwitchCommand::Switch),
            ("!gif", _) => Some(TwitchCommand::Gif),
            ("!sound", _) => Some(TwitchCommand::Sound),
            ("!wordstonks", _) => Some(TwitchCommand::WordStonks),
            ("!wordguess", _) => Some(TwitchCommand::WordGuess),
            ("!wg", _) => Some(TwitchCommand::WordGuess),
            _ => None,
        }
    }
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

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
