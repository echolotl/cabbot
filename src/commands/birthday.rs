use chrono::{Datelike, NaiveDate, Utc};
use log::{debug, info, warn};
use serenity::{
    all::{
        CommandInteraction, CommandOptionType, Context, CreateInteractionResponse,
        CreateInteractionResponseMessage, Permissions, ResolvedOption, ResolvedValue,
    },
    builder::{CreateCommand, CreateCommandOption},
};

use crate::{
    commands::CabbotCommandError,
    utils::{
        data::{Birthday, Data, GuildMutationLocks, Settings, read_lock, write_lock},
        discord::user_lookup,
    },
};

static MONTH_DECORATIONS: [&str; 12] = ["✫", "▰", "❀", "ꕤ", "ღ", "⚣", "✹", "❦", "▨", "֎", "✧", "❅"];
static MONTH_NAMES: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

/// BIRTHDAY COMMAND
/// /birthday [add|remove|list]
pub fn register() -> CreateCommand {
    CreateCommand::new("birthday")
        .description("Add or remove birthday alerts for members")
        .default_member_permissions(Permissions::ADMINISTRATOR)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "add",
                "Add a birthday alert for a member",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::User,
                    "user",
                    "The user whose birthday it is",
                )
                .required(true),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "month",
                    "The month of the user's birthday",
                )
                .required(true),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "day",
                    "The day of the user's birthday",
                )
                .required(true),
            ),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "remove",
                "Remove a birthday alert for a member",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::User,
                    "user",
                    "The user whose birthday alert should be removed",
                )
                .required(true),
            ),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "list",
            "List all birthday alerts for members",
        ))
}

/// Runs the actual birthday command.
pub async fn run(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<(), CabbotCommandError> {
    let resolved_options = interaction.data.options();
    let Some(ResolvedOption {
        value: ResolvedValue::SubCommand(options),
        name: subcommand_name,
        ..
    }) = resolved_options.first()
    else {
        return Err(CabbotCommandError::MissingOption);
    };
    let guild_id = interaction
        .guild_id
        .ok_or(CabbotCommandError::GuildOnly)?
        .get();
    let (birthdays, settings, guild_locks) = {
        let data = ctx.data.read().await;
        (
            data.get::<Data>()
                .cloned()
                .ok_or(CabbotCommandError::MissingState)?,
            data.get::<Settings>()
                .cloned()
                .ok_or(CabbotCommandError::MissingState)?,
            data.get::<GuildMutationLocks>()
                .cloned()
                .ok_or(CabbotCommandError::MissingState)?,
        )
    };

    match *subcommand_name {
        "add" => {
            require_admin(interaction.member.as_ref().and_then(|m| m.permissions))?;
            let user = options
                .iter()
                .find(|option| option.name == "user")
                .ok_or_else(|| CabbotCommandError::MissingOption)?;
            let ResolvedValue::User(user, _) = user.value else {
                return Err(CabbotCommandError::InvalidUser);
            };

            let month = options
                .iter()
                .find(|option| option.name == "month")
                .ok_or_else(|| CabbotCommandError::MissingOption)?;
            let ResolvedValue::Integer(month) = month.value else {
                return Err(CabbotCommandError::InvalidDate);
            };
            let month = u32::try_from(month).map_err(|_| CabbotCommandError::InvalidDate)?;

            let day = options
                .iter()
                .find(|option| option.name == "day")
                .ok_or_else(|| CabbotCommandError::MissingOption)?;
            let ResolvedValue::Integer(day) = day.value else {
                return Err(CabbotCommandError::InvalidDate);
            };
            let day = u32::try_from(day).map_err(|_| CabbotCommandError::InvalidDate)?;

            let date = next_birthday(month, day).ok_or_else(|| CabbotCommandError::InvalidDate)?;
            let user_id = user.id.get();
            let _guild_lock = guild_locks.for_guild(guild_id).lock_owned().await;
            let existing = { read_lock(&birthdays).get(&guild_id).cloned() };
            let mut data = match existing {
                Some(data) => data,
                None => Data::get_async(guild_id)
                    .await
                    .map_err(CabbotCommandError::unknown)?,
            };
            let message = if data.birthdays.iter().any(|b| b.user_id == user_id) {
                warn!("Birthday already exists for user {user_id} in guild {guild_id}");
                "Birthday already set.".to_string()
            } else {
                data.birthdays.push(Birthday {
                    user_id,
                    month,
                    day,
                });
                data.save_async(guild_id)
                    .await
                    .map_err(|error| CabbotCommandError::save_failed("birthdays", error))?;
                write_lock(&birthdays).insert(guild_id, data);
                info!("Added birthday for user {user_id} ({month}/{day}) in guild {guild_id}");
                format!(
                    "**{}**'s birthday has been added for {}!",
                    user.name,
                    format_birthday_date(date)
                )
            };
            drop(_guild_lock);
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
                .map_err(CabbotCommandError::response_failed)?;
            Ok(())
        }
        "remove" => {
            require_admin(interaction.member.as_ref().and_then(|m| m.permissions))?;
            let user = options
                .iter()
                .find(|option| option.name == "user")
                .ok_or_else(|| CabbotCommandError::MissingOption)?;
            let ResolvedValue::User(user, _) = user.value else {
                return Err(CabbotCommandError::InvalidUser);
            };

            let user_id = user.id.get();
            let _guild_lock = guild_locks.for_guild(guild_id).lock_owned().await;
            let existing = { read_lock(&birthdays).get(&guild_id).cloned() };
            let mut data = match existing {
                Some(data) => data,
                None => Data::get_async(guild_id)
                    .await
                    .map_err(CabbotCommandError::unknown)?,
            };
            let message =
                if let Some(pos) = data.birthdays.iter().position(|b| b.user_id == user_id) {
                    data.birthdays.remove(pos);
                    data.save_async(guild_id)
                        .await
                        .map_err(|error| CabbotCommandError::save_failed("birthdays", error))?;
                    write_lock(&birthdays).insert(guild_id, data);
                    info!("Removed birthday for user {user_id} in guild {guild_id}");
                    format!("**{}**'s birthday has been removed.", user.name)
                } else {
                    warn!("No birthday found for user {user_id} in guild {guild_id}");
                    format!("**{}** does not have a birthday set.", user.name)
                };
            drop(_guild_lock);
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
                .map_err(CabbotCommandError::response_failed)?;
            Ok(())
        }
        "list" => {
            debug!("Listing birthdays for guild {guild_id}");
            let birthdays = read_lock(&birthdays)
                .get(&guild_id)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| Data::get(guild_id))
                .map_err(CabbotCommandError::unknown)?
                .birthdays;
            // Don't let the user list birthdays in any other channel than the birthday channel
            let birthday_channel = read_lock(&settings)
                .get(&guild_id)
                .map(|guild_settings| guild_settings.birthday_channel)
                .unwrap_or_else(|| Settings::default().birthday_channel);
            if interaction.channel_id.get() != birthday_channel {
                return Err(CabbotCommandError::WrongChannel(birthday_channel));
            }
            let message = list(ctx, birthdays).await?;
            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(message),
                    ),
                )
                .await
                .map_err(CabbotCommandError::response_failed)?;
            Ok(())
        }
        _ => return Err(CabbotCommandError::UnknownAction),
    }
}

fn require_admin(permissions: Option<Permissions>) -> Result<(), CabbotCommandError> {
    if permissions.is_some_and(|permissions| permissions.contains(Permissions::ADMINISTRATOR)) {
        Ok(())
    } else {
        Err(CabbotCommandError::InsufficientPermissions)
    }
}

async fn list(ctx: &Context, birthdays: Vec<Birthday>) -> Result<String, CabbotCommandError> {
    if birthdays.is_empty() {
        return Ok("No birthdays are set.".to_string());
    }

    let mut message = String::new();
    // Group birthdays by month
    let mut birthdays_by_month: [Vec<&Birthday>; 12] = Default::default();
    for birthday in &birthdays {
        if next_birthday(birthday.month, birthday.day).is_none() {
            return Err(CabbotCommandError::InvalidDate);
        }
        birthdays_by_month[(birthday.month - 1) as usize].push(birthday);
    }

    for (month_index, month_birthdays) in birthdays_by_month.iter().enumerate() {
        if !month_birthdays.is_empty() {
            message.push_str(&format!(
                "## {} {}:\n",
                MONTH_DECORATIONS[month_index], MONTH_NAMES[month_index]
            ));

            let dates = month_birthdays
                .iter()
                .map(|birthday| {
                    next_birthday(birthday.month, birthday.day)
                        .ok_or(CabbotCommandError::InvalidDate)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let user_ids = month_birthdays
                .iter()
                .map(|birthday| birthday.user_id)
                .collect();
            let users = user_lookup(ctx, user_ids).await?;
            for (user, date) in users.iter().zip(dates) {
                message.push_str(&format!(
                    "**{}**: {}\n",
                    user.name,
                    format_birthday_date(date)
                ));
            }
        }
    }
    Ok(message)
}

fn format_birthday_date(date: NaiveDate) -> String {
    fn ordinal_suffix(n: u32) -> &'static str {
        match (n % 100, n % 10) {
            (11..=13, _) => "th",
            (_, 1) => "st",
            (_, 2) => "nd",
            (_, 3) => "rd",
            _ => "th",
        }
    }
    format!("{}{}", date.format("%B %-d"), ordinal_suffix(date.day()))
}

fn next_birthday(month: u32, day: u32) -> Option<NaiveDate> {
    next_birthday_on(Utc::now().date_naive(), month, day)
}

fn next_birthday_on(today: NaiveDate, month: u32, day: u32) -> Option<NaiveDate> {
    let birthday_in_year = |year| {
        NaiveDate::from_ymd_opt(year, month, day).or_else(|| {
            if (month, day) == (2, 29) {
                NaiveDate::from_ymd_opt(year, 2, 28)
            } else {
                None
            }
        })
    };
    let date = birthday_in_year(today.year())?;
    if date < today {
        birthday_in_year(today.year().checked_add(1)?)
    } else {
        Some(date)
    }
}
