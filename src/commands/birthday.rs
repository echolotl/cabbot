use chrono::{Datelike, NaiveDate, Utc};
use serenity::{
    all::{
        CommandInteraction, CommandOptionType, Context, CreateInteractionResponse,
        CreateInteractionResponseMessage, Permissions, ResolvedOption, ResolvedValue, UserId,
    },
    builder::{CreateCommand, CreateCommandOption},
};

use crate::{Globals, commands::CabbotCommandError, utils::data::Birthday};

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
    let globals = Globals::get().await.lock().await;
    let Some(ResolvedOption {
        value: ResolvedValue::SubCommand(options),
        name: subcommand_name,
        ..
    }) = resolved_options.first()
    else {
        return Err(CabbotCommandError::MissingOption);
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

            let mut data = globals.data.lock().await;
            if data.birthdays.iter().any(|b| b.user_id == user_id) {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Birthday already set.")
                                .ephemeral(true),
                        ),
                    )
                    .await
                    .map_err(CabbotCommandError::response_failed)?;
                return Ok(());
            }
            let mut updated = data.clone();
            updated.birthdays.push(Birthday {
                user_id,
                month,
                day,
            });
            updated
                .save()
                .map_err(|error| CabbotCommandError::save_failed("birthdays", error))?;
            *data = updated;
            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!(
                                "**{}**'s birthday has been added for <t:{}:D>!",
                                user.name,
                                date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
                            ))
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
            let mut data = globals.data.lock().await;
            if let Some(pos) = data.birthdays.iter().position(|b| b.user_id == user_id) {
                let mut updated = data.clone();
                updated.birthdays.remove(pos);
                updated
                    .save()
                    .map_err(|error| CabbotCommandError::save_failed("birthdays", error))?;
                *data = updated;
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("**{}**'s birthday has been removed.", user.name))
                                .ephemeral(true),
                        ),
                    )
                    .await
                    .map_err(CabbotCommandError::response_failed)?;
                Ok(())
            } else {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("**{}** does not have a birthday set.", user.name))
                                .ephemeral(true),
                        ),
                    )
                    .await
                    .map_err(CabbotCommandError::response_failed)?;
                Ok(())
            }
        }
        "list" => {
            let birthdays = globals.data.lock().await.birthdays.clone();
            // Don't let the user list birthdays in any other channel than the birthday channel
            let birthday_channel = globals.settings.lock().await.birthday_channel;
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
            for birthday in month_birthdays {
                let date = next_birthday(birthday.month, birthday.day)
                    .ok_or(CabbotCommandError::InvalidDate)?;
                let user = UserId::new(birthday.user_id)
                    .to_user(ctx)
                    .await
                    .map_err(|error| {
                        eprintln!("Error looking up user: {}", error);
                        CabbotCommandError::LookupFailed("user".to_string())
                    })?;
                message.push_str(&format!(
                    "**{}**: <t:{}:D>\n",
                    user.display_name(),
                    date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
                ));
            }
        }
    }
    Ok(message)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_birthday_handles_leap_days_and_invalid_dates() {
        let date = |year, month, day| NaiveDate::from_ymd_opt(year, month, day).unwrap();
        assert_eq!(
            next_birthday_on(date(2025, 1, 1), 2, 29),
            Some(date(2025, 2, 28))
        );
        assert_eq!(
            next_birthday_on(date(2027, 3, 1), 2, 29),
            Some(date(2028, 2, 29))
        );
        assert_eq!(
            next_birthday_on(date(2024, 3, 1), 2, 29),
            Some(date(2025, 2, 28))
        );
        assert_eq!(next_birthday_on(date(2025, 1, 1), 2, 30), None);
        assert_eq!(next_birthday_on(date(2025, 1, 1), 0, 1), None);
    }
}
