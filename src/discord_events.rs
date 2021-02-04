pub struct Handler;

use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::fs::File;
use std::io::prelude::*;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "?testingtesting" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "one two, one two") {
                println!("Error giving message: {:?}", why)
            }
        }
    }
    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is ready", ready.user.name);
    }
}

pub fn activate_discord_bot() {
    let mut file = File::open(".token").expect("Error loading Discord token");
    let mut token = String::new();
    file.read_to_string(&mut token)
        .expect("Token file not found");
    let mut client = Client::new(&token, Handler).expect("Error creating client");
    if let Err(msg) = client.start() {
        println!("Error: {:?}", msg);
    }
}
