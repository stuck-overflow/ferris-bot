use async_trait::async_trait;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use structopt::StructOpt;
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage, UserAccessToken};
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::ClientConfig;
use twitch_irc::TCPTransport;
use twitch_irc::TwitchIRCClient;

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

#[derive(Debug)]
struct CustomTokenStorage {
    last_token_json: Option<String>,
    token_checkpoint_file: String,
}

#[async_trait]
impl TokenStorage for CustomTokenStorage {
    type LoadError = std::io::Error; // or some other error
    type UpdateError = std::io::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        println!("load_token called");
        match &self.last_token_json {
            Some(ref access_token) => Ok(serde_json::from_str(access_token).unwrap()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "token doesn't exist",
            )),
        }
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        println!("update_token called");
        // Called after the token was updated successfully, to save the new token.
        // After `update_token()` completes, the `load_token()` method should then return
        // that token for future invocations
        self.last_token_json = Some(serde_json::to_string(&token).unwrap());
        // TODO WRITE TO FILE
<<<<<<< HEAD
        
=======
>>>>>>> 95e1da9d32961913f5d59b7d2215df1b2547ba05
        Ok(())
    }
}

#[derive(Deserialize)]
struct TwitchAuth {
<<<<<<< HEAD
    token_path: String,
=======
    token_filepath: String,
>>>>>>> 95e1da9d32961913f5d59b7d2215df1b2547ba05
    login_name: String,
    client_id: String,
    secret: String,
}

<<<<<<< HEAD
#[tokio::main]
pub async fn main() {
    let twitch_auth = fs::read_to_string("twitchauth.toml").unwrap();
    let twitch_auth: TwitchAuth = toml::from_str(&twitch_auth).unwrap();
    println!("twitch_auth read?");
    let last_token_json = Path::new(&twitch_auth.token_path);
    let storage = if last_token_json.is_file() {
        println!("{} is a file", twitch_auth.token_path);
        let token = fs::read_to_string(&twitch_auth.token_path).unwrap();
        CustomTokenStorage { last_token_json: Some(token), token_checkpoint_file: twitch_auth.token_path  }
    } else {
        println!("{} is not a file", twitch_auth.token_path);
        CustomTokenStorage { last_token_json: Some(String::from(&twitch_auth.secret)), token_checkpoint_file: twitch_auth.token_path } 
    };    

    let irc_config = ClientConfig::new_simple(
        RefreshingLoginCredentials::new(twitch_auth.login_name, twitch_auth.client_id, twitch_auth.secret, storage)
    );
    let (mut incoming_messages, client) =
        TwitchIRCClient::<TCPTransport, RefreshingLoginCredentials<CustomTokenStorage>>::new(irc_config);
=======
// Command-line arguments for the tool.
#[derive(StructOpt)]
struct Cli {
    /// Twitch credential files.
    #[structopt(short, long, default_value = "twitchauth.toml")]
    credentials_file: String,

    /// Generates the curl command to obtain the first token and exits.
    #[structopt(short, long)]
    generate_curl_first_token_request: bool,

    /// Auth code to be used when obtaining first token.
    #[structopt(long, default_value = "")]
    auth_code: String,

    /// Show the authentication URL and exits.
    #[structopt(short, long)]
    show_auth_url: bool,
}

#[tokio::main]
pub async fn main() {
    let args = Cli::from_args();

    let twitch_auth = fs::read_to_string(args.credentials_file).unwrap();
    let twitch_auth: TwitchAuth = toml::from_str(&twitch_auth).unwrap();

    if args.show_auth_url {
        println!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost&response_type=code&scope=chat:read%20chat:edit", twitch_auth.client_id);
        std::process::exit(0);
    }

    if args.generate_curl_first_token_request {
        if args.auth_code.is_empty() {
            println!("Please set --auth_code. Aborting.");
            std::process::exit(1);
        }
        println!("curl -X POST 'https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&code={}&grant_type=authorization_code&redirect_uri=http://localhost' > firsttoken.json",
            twitch_auth.client_id,
            twitch_auth.secret,
            args.auth_code);
        std::process::exit(0);
    }

    let last_token_json = Path::new(&twitch_auth.token_filepath);
    let storage = if last_token_json.is_file() {
        println!("{} is a file", twitch_auth.token_filepath);
        let token = fs::read_to_string(&twitch_auth.token_filepath).unwrap();
        CustomTokenStorage {
            last_token_json: Some(token),
            token_checkpoint_file: twitch_auth.token_filepath,
        }
    } else {
        println!("{} is not a file", twitch_auth.token_filepath);
        CustomTokenStorage {
            last_token_json: Some(String::from(&twitch_auth.secret)),
            token_checkpoint_file: twitch_auth.token_filepath,
        }
    };

    let irc_config = ClientConfig::new_simple(RefreshingLoginCredentials::new(
        twitch_auth.login_name,
        twitch_auth.client_id,
        twitch_auth.secret,
        storage,
    ));
    let (mut incoming_messages, client) = TwitchIRCClient::<
        TCPTransport,
        RefreshingLoginCredentials<CustomTokenStorage>,
    >::new(irc_config);
>>>>>>> 95e1da9d32961913f5d59b7d2215df1b2547ba05

    // first thing you should do: start consuming incoming messages,
    // otherwise they will back up.
    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => parse_command(msg),
                _ => println!("{:?}: received", message),
            }
        }
    });

    // join a channel
    client.join("stuck_overflow".to_owned());

    // keep the tokio executor alive.
    // If you return instead of waiting the background task will exit.
    join_handle.await.unwrap();
}
