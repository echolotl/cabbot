mod commands;
mod utils;

use serenity::{
    all::{
        ActivityData, CommandInteraction, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage, GatewayIntents, Guild, GuildId, Interaction, OnlineStatus,
    }, async_trait, prelude::*,
};
use std::env;
use std::sync::{Arc, RwLock};
use utils::data::{Data, Settings};

use crate::commands::CabbotCommandError;

fn commands_to_register() -> Vec<CreateCommand> {
    vec![
        commands::birthday::register(),
        commands::settings::register(),
        commands::about::register(),
    ]
}

struct Cabbot {
    guild_id: GuildId,
}

#[async_trait]
impl EventHandler for Cabbot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
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
                    eprintln!("Autocomplete for {} failed: {error:?}", command.data.name);
                }
            }
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        println!("{} is connected!", ready.user.name);
        if let Err(error) = self.guild_id
            .set_commands(
                &ctx.http,
                commands_to_register()
            )
            .await
        {
            eprintln!("Failed to register guild commands: {error}");
        }
        ctx.set_presence(Some(ActivityData::custom("(i'm cozy)")), OnlineStatus::Idle);
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
        if Some(true) == is_new {
            self.guild_id.set_commands(&ctx.http, commands_to_register()).await.unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
    let guild_id = env::var("GUILD_ID")
        .expect("GUILD_ID must be set")
        .parse::<u64>()
        .expect("GUILD_ID must be a valid guild ID");
    let data = Arc::new(RwLock::new(
        Data::get().expect("Failed to load birthday data"),
    ));
    let settings = Arc::new(RwLock::new(
        Settings::get().expect("Failed to load settings"),
    ));
    let mut client = Client::builder(&discord_token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(Cabbot {
            guild_id: GuildId::new(guild_id),
        })
        .type_map_insert::<Data>(Arc::clone(&data))
        .type_map_insert::<Settings>(Arc::clone(&settings))
        .await
        .expect("Error creating client");
    let scheduler = utils::scheduler::start(data, settings, Arc::clone(&client.http));

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
