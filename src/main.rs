mod commands;
mod utils;

use colored::Colorize;
use log::{debug, error, info, trace, warn};
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
use crate::utils::data::{Data, GuildMutationLocks, Settings, read_lock};

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
                    "{} command ran by user {}",
                    format!("/{}", command.data.name).bright_yellow(), command.user.name
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
        info!("{} is connected!", ready.user.name.to_string().bright_yellow());
        let mut guilds_set: usize = 0;
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
                guilds_set += 1;
            }
        }
        info!("Registered commands for {guilds_set} guilds");
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
    fern::Dispatch::new().level(log::LevelFilter::Warn).level_for("cabbot", if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    }).format(|out, message, record| {
        out.finish(format_args!(
            "{} [{}] {}",
            format_log_header(record.level()),
            record.module_path().unwrap_or("").bright_black(),
            message
        ))
    })
    .chain(
        fern::Dispatch::new()
            .filter(|metadata| metadata.level() == log::Level::Error)
            .chain(std::io::stderr()),
    )
    .chain(
        fern::Dispatch::new()
            .filter(|metadata| metadata.level() != log::Level::Error)
            .chain(std::io::stdout()),
    )
    .apply()
    .unwrap();
    info!("Starting Cabbot!");
    let discord_token = env::var("DISCORD_TOKEN").unwrap_or_else(|error| {
        error!("DISCORD_TOKEN is unavailable: {error}");
        panic!("DISCORD_TOKEN must be set");
    });
    info!("Loading settings and  data...");
    let data = Arc::new(RwLock::new(Data::get_all().unwrap_or_else(|error| {
        error!("Failed to load data: {error}");
        panic!("Failed to load data");
    })));
    let settings = Arc::new(RwLock::new(Settings::get_all().unwrap_or_else(|error| {
        error!("Failed to load settings: {error}");
        panic!("Failed to load settings");
    })));
    info!(
        "Loaded {} data entries and {} settings entries!",
        read_lock(&data).len().to_string().bright_yellow(),
        read_lock(&settings).len().to_string().bright_yellow()
    );
    let mut client = Client::builder(&discord_token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(Cabbot {})
        .type_map_insert::<Data>(Arc::clone(&data))
        .type_map_insert::<Settings>(Arc::clone(&settings))
        .type_map_insert::<GuildMutationLocks>(Arc::new(GuildMutationLocks::default()))
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
    info!("Connecting to Discord...");
    if let Err(error) = client.start().await {
        error!("Discord client stopped with an error: {error:?}");
    }
    scheduler.stop().await;
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

fn format_log_header(level: log::Level) -> String {
    match level {
        log::Level::Debug => " DEBUG ".on_purple().white().bold().to_string(),
        log::Level::Info => format!(" {}", " INFO ".on_blue().black().bold()),
        log::Level::Warn => format!(" {}", " WARN ".on_bright_yellow().black().bold()),
        log::Level::Error => " ERROR ".on_red().black().bold().to_string(),
        log::Level::Trace => " TRACE ".on_bright_magenta().black().bold().to_string(),
    }
}
