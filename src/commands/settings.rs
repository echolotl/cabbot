use serenity::{
    all::{
        ChannelType, CommandInteraction, CommandOptionType, Context, CreateInteractionResponse,
        CreateInteractionResponseMessage, Permissions, ResolvedOption, ResolvedValue,
    },
    builder::{CreateAutocompleteResponse, CreateCommand, CreateCommandOption},
};

use crate::{Globals, commands::CabbotCommandError};

/// SETTINGS COMMAND
/// /settings [modify|view]
pub fn register() -> CreateCommand {
    CreateCommand::new("settings")
        .description("Edit various settings for the bot")
        .default_member_permissions(Permissions::ADMINISTRATOR)
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "modify", "Modify a setting")
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "setting",
                        "The setting to modify",
                    )
                    .required(true)
                    .add_string_choice("Birthday Channel", "birthday_channel"),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "value",
                        "The text channel for birthday announcements",
                    )
                    .required(true)
                    .set_autocomplete(true),
                ),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "view",
            "View current settings",
        ))
}

pub async fn run(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CabbotCommandError> {
    if !interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.contains(Permissions::ADMINISTRATOR))
    {
        return Err(CabbotCommandError::InsufficientPermissions);
    }

    let resolved_options = interaction.data.options();
    let Some(ResolvedOption {
        name: subcommand,
        value: ResolvedValue::SubCommand(options),
        ..
    }) = resolved_options.first()
    else {
        return Err(CabbotCommandError::MissingOption);
    };

    let settings = {
        let globals = Globals::get().await.lock().await;
        globals.settings.clone()
    };

    let message = match *subcommand {
        "modify" => {
            let setting = options
                .iter()
                .find(|option| option.name == "setting")
                .ok_or(CabbotCommandError::MissingOption)?;
            if !matches!(setting.value, ResolvedValue::String("birthday_channel")) {
                return Err(CabbotCommandError::UnknownAction);
            }
            let value = options
                .iter()
                .find(|option| option.name == "value")
                .ok_or(CabbotCommandError::MissingOption)?;
            let ResolvedValue::String(value) = value.value else {
                return Err(CabbotCommandError::InvalidTextChannel);
            };
            let channel_id = value
                .parse::<u64>()
                .map_err(|_| CabbotCommandError::InvalidTextChannel)?;
            let guild_id = interaction
                .guild_id
                .ok_or(CabbotCommandError::InvalidTextChannel)?;
            let channels = guild_id.channels(&ctx.http).await.map_err(|error| {
                eprintln!("Error looking up channel: {}", error);
                CabbotCommandError::LookupFailed("channel".to_string())
            })?;
            if !channels
                .values()
                .any(|channel| channel.id.get() == channel_id && channel.kind == ChannelType::Text)
            {
                return Err(CabbotCommandError::InvalidTextChannel);
            }

            let mut settings = settings.lock().await;
            let mut updated = settings.clone();
            updated.birthday_channel = channel_id;
            updated
                .save()
                .map_err(|error| CabbotCommandError::save_failed("settings", error))?;
            *settings = updated;
            format!("Birthday announcements will be sent to <#{channel_id}>.")
        }
        "view" => {
            let channel_id = settings.lock().await.birthday_channel;
            if channel_id == 0 {
                "**BIRTHDAY CHANNEL**: Not set.".to_string()
            } else {
                format!("**BIRTHDAY CHANNEL**: <#{channel_id}>")
            }
        }
        _ => return Err(CabbotCommandError::UnknownAction),
    };

    interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(message)
                    .ephemeral(true),
            ),
        )
        .await
        .map_err(CabbotCommandError::response_failed)
}

/// idk why discord doesn't do like a CommandOptionType::TextChannel...
pub async fn autocomplete(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CabbotCommandError> {
    let focused = interaction
        .data
        .autocomplete()
        .ok_or(CabbotCommandError::MissingOption)?;
    if focused.name != "value" {
        return Err(CabbotCommandError::UnknownAction);
    }
    let guild_id = interaction
        .guild_id
        .ok_or(CabbotCommandError::InvalidTextChannel)?;
    let channels = guild_id.channels(&ctx.http).await.map_err(|error| {
        eprintln!("Error looking up channel: {}", error);
        CabbotCommandError::LookupFailed("channel".to_string())
    })?;
    let query = focused.value.trim_start_matches('#').to_lowercase();
    let mut channels: Vec<_> = channels
        .values()
        .filter(|channel| {
            channel.kind == ChannelType::Text
                && (channel.name.to_lowercase().contains(&query)
                    || channel.id.to_string().contains(&query))
        })
        .collect();
    channels.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    let mut response = CreateAutocompleteResponse::new();
    for channel in channels.into_iter().take(25) {
        response = response.add_string_choice(&channel.name, channel.id.to_string());
    }
    interaction
        .create_response(ctx, CreateInteractionResponse::Autocomplete(response))
        .await
        .map_err(CabbotCommandError::response_failed)
}
