mod commands;
mod utils;

use serenity::{
    all::{
        CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage, GuildId,
        Interaction,
    },
    async_trait,
    prelude::*,
};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::OnceCell;
use utils::data::{Data, Settings};

use crate::commands::CabbotCommandError;

/// Global state for the bot
struct Globals {
    guild_id: GuildId,
    client: Option<Client>,
    data: Arc<AsyncMutex<Data>>,
    settings: Arc<AsyncMutex<Settings>>,
}

impl Globals {
    async fn get() -> &'static AsyncMutex<Globals> {
        GLOBALS
            .get_or_init(|| async { AsyncMutex::new(Self::init().await) })
            .await
    }

    #[inline]
    async fn init() -> Globals {
        let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
        let guild_id = env::var("GUILD_ID")
            .expect("GUILD_ID must be set")
            .parse::<u64>()
            .expect("GUILD_ID must be a valid guild ID");
        let client = Client::builder(&discord_token, GatewayIntents::GUILD_MEMBERS)
            .event_handler(Handler)
            .await
            .expect("Error creating client");
        Globals {
            guild_id: GuildId::new(guild_id),
            client: Some(client),
            data: Arc::new(AsyncMutex::new(
                Data::get().expect("Failed to load birthday data"),
            )),
            settings: Arc::new(AsyncMutex::new(
                Settings::get().expect("Failed to load settings"),
            )),
        }
    }
}

static GLOBALS: OnceCell<AsyncMutex<Globals>> = OnceCell::const_new();

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                let result = match command.data.name.as_str() {
                    "birthday" => commands::birthday::run(&ctx, &command).await,
                    "settings" => commands::settings::run(&ctx, &command).await,
                    _ => Err(CabbotCommandError::UnknownAction),
                };
                if let Err(error) = result {
                    handle_bot_error(&ctx, &command, error).await;
                }
            }
            Interaction::Autocomplete(command) => {
                let result = match command.data.name.as_str() {
                    "settings" => commands::settings::autocomplete(&ctx, &command).await,
                    _ => Err(CabbotCommandError::UnknownAction),
                };
                if let Err(error) = result {
                    eprintln!("Autocomplete for {} failed: {error:?}", command.data.name);
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        println!("{} is connected!", ready.user.name);
        let guild_id = Globals::get().await.lock().await.guild_id;
        if let Err(error) = guild_id
            .set_commands(
                &ctx.http,
                vec![
                    commands::birthday::register(),
                    commands::settings::register(),
                ],
            )
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
    let mut globals_guard = globals.lock().await;
    let mut client = {
        let client = globals_guard
            .client
            .take()
            .expect("Discord client was already started");
        client
    };
    let scheduler = utils::scheduler::start(&*globals_guard, Arc::clone(&client.http));
    drop(globals_guard);

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
    scheduler.stop();
}

async fn handle_bot_error(
    ctx: &Context,
    interaction: &CommandInteraction,
    error: CabbotCommandError,
) {
    eprintln!("Command {} failed: {error:?}", interaction.data.name);
    if let Err(response_error) = interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("{}", error))
                    .ephemeral(true),
            ),
        )
        .await
    {
        eprintln!("Failed to report command error ({error:?}): {response_error}");
    }
}
