mod commands;
mod utils;

use log::{debug, error, info};
use serenity::{
    all::{
        ActivityData, CommandInteraction, CreateCommand, CreateInteractionResponse,
        CreateInteractionResponseMessage, GatewayIntents, Guild, Interaction, OnlineStatus,
    },
    async_trait,
    prelude::*,
};
use std::env;
use std::sync::{Arc, RwLock};

use crate::commands::CabbotCommandError;
use crate::utils::data::{Data, Settings, read_lock};

fn commands_to_register() -> Vec<CreateCommand> {
    vec![
        commands::birthday::register(),
        commands::settings::register(),
        commands::about::register(),
    ]
}

struct Cabbot {}

#[async_trait]
impl EventHandler for Cabbot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                info!(
                    "Received /{} command from user {} in guild {:?}",
                    command.data.name, command.user.id, command.guild_id
                );
                let result = match command.data.name.as_str() {
                    "birthday" => commands::birthday::run(&ctx, &command).await,
                    "settings" => commands::settings::run(&ctx, &command).await,
                    "about" => commands::about::run(&ctx, &command).await,
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
                    error!("Autocomplete for {} failed: {error:?}", command.data.name);
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        info!("{} is connected", ready.user.name);
        for guild in ready.guilds {
            if let Err(error) = guild
                .id
                .set_commands(&ctx.http, commands_to_register())
                .await
            {
                error!(
                    "Failed to register commands for guild {}: {error}",
                    guild.id
                );
            } else {
                debug!("Registered application commands for guild {}", guild.id);
            }
        }
        ctx.set_presence(Some(ActivityData::custom("(i'm cozy)")), OnlineStatus::Idle);
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, _is_new: Option<bool>) {
        // Also register commands for new guilds
        info!("Joined guild {}", guild.id);
        if let Err(error) = guild
            .id
            .set_commands(&ctx.http, commands_to_register())
            .await
        {
            error!(
                "Failed to register commands for guild {}: {error}",
                guild.id
            );
        } else {
            debug!("Registered application commands for guild {}", guild.id);
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    info!("Starting Cabbot!");
    let discord_token = env::var("DISCORD_TOKEN").unwrap_or_else(|error| {
        error!("DISCORD_TOKEN is unavailable: {error}");
        panic!("DISCORD_TOKEN must be set");
    });
    info!("Loading settings and birthday data");
    let data = Arc::new(RwLock::new(Data::get_all().unwrap_or_else(|error| {
        error!("Failed to load birthday data: {error}");
        panic!("Failed to load birthday data");
    })));
    let settings = Arc::new(RwLock::new(Settings::get_all().unwrap_or_else(|error| {
        error!("Failed to load settings: {error}");
        panic!("Failed to load settings");
    })));
    info!(
        "Loaded birthday data and settings for {} and {} guilds",
        read_lock(&data).len(),
        read_lock(&settings).len()
    );
    let mut client = Client::builder(&discord_token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(Cabbot {})
        .type_map_insert::<Data>(Arc::clone(&data))
        .type_map_insert::<Settings>(Arc::clone(&settings))
        .await
        .unwrap_or_else(|error| {
            error!("Failed to create Discord client: {error}");
            panic!("Error creating client");
        });
    let scheduler = utils::scheduler::start(
        Arc::clone(&data),
        Arc::clone(&settings),
        Arc::clone(&client.http),
    );

    info!("Connecting to Discord");
    if let Err(error) = client.start().await {
        error!("Discord client stopped with an error: {error:?}");
    }
    scheduler.stop();
    info!("Cabbot shut down");
}

async fn handle_bot_error(
    ctx: &Context,
    interaction: &CommandInteraction,
    error: CabbotCommandError,
) {
    error!("Command /{} failed: {error:?}", interaction.data.name);
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
        error!("Failed to report command error ({error:?}): {response_error}");
    }
}
