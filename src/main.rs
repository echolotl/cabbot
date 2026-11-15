mod commands;
mod utils;

use serenity::{
    all::{CreateInteractionResponse, CreateInteractionResponseMessage, GuildId, Interaction},
    async_trait,
    prelude::*,
};
use std::env;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::OnceCell;
use utils::data::Data;

/// Global state for the application, including the Discord client and token.
struct Globals {
    guild_id: String,
    client: Option<Client>,
    data: Arc<AsyncMutex<Data>>,
}

impl Globals {
    async fn get() -> &'static Mutex<Globals> {
        GLOBALS
            .get_or_init(|| async { Mutex::new(init().await) })
            .await
    }
}

static GLOBALS: OnceCell<Mutex<Globals>> = OnceCell::const_new();

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let options = command.data.options();
            // Anyone can run the birthday list command (and everyone can see it too)
            // TODO: Make this not hardcoded to the /birthday list if I end up adding more...
            let ephemeral = !(command.data.name == "birthday"
                && options.first().is_some_and(|option| option.name == "list"));
            let permissions = command
                .member
                .as_ref()
                .and_then(|member| member.permissions);
            let response = match command.data.name.as_str() {
                "birthday" => {
                    let data = {
                        let globals = Globals::get().await.lock().unwrap();
                        Arc::clone(&globals.data)
                    };
                    match commands::birthday::run(&options, &data, permissions, &ctx.http).await {
                        Ok(message) => Ok(message),
                        Err(error) => Err(error.to_string()),
                    }
                }
                _ => Err("Unknown command.".to_string()),
            };
            if let Err(error) = command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(match response {
                                Ok(message) => message,
                                Err(error) => error,
                            })
                            .ephemeral(ephemeral),
                    ),
                )
                .await
            {
                eprintln!("Failed to respond to command: {error}");
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        println!("{} is connected!", ready.user.name);
        let guild_id = {
            let globals = Globals::get().await;
            let globals = globals.lock().unwrap();
            globals.guild_id.parse::<u64>()
        };
        let guild_id = match guild_id {
            Ok(guild_id) => GuildId::new(guild_id),
            Err(error) => {
                eprintln!("Invalid GUILD_ID: {error}");
                return;
            }
        };
        if let Err(error) = guild_id
            .set_commands(&ctx.http, vec![commands::birthday::register()])
            .await
        {
            eprintln!("Failed to register guild commands: {error}");
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let globals = Globals::get().await;
    let mut client = {
        let mut globals = globals.lock().unwrap();
        globals
            .client
            .take()
            .expect("Discord client was already started")
    };

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}

#[inline]
async fn init() -> Globals {
    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
    let guild_id = env::var("GUILD_ID").expect("GUILD_ID must be set");
    let client = Client::builder(&discord_token, GatewayIntents::empty())
        .event_handler(Handler)
        .await
        .expect("Error creating client");
    Globals {
        guild_id,
        client: Some(client),
        data: Arc::new(AsyncMutex::new(
            Data::get().expect("Failed to load birthday data"),
        )),
    }
}
