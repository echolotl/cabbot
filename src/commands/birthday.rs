use chrono::Datelike;
use serenity::{
    all::{CommandOptionType, Http, Permissions, ResolvedOption, ResolvedValue, UserId},
    builder::{CreateCommand, CreateCommandOption},
};
use tokio::sync::Mutex;

use crate::utils::data::Data;

/// Errors that can occur when performing birthday actions.
pub enum BirthdayActionError {
    InvalidUser,
    InvalidDate,
    MissingOption,
    UnknownAction,
    InsufficientPermissions,
    UserLookupFailed(String),
}

impl std::fmt::Display for BirthdayActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BirthdayActionError::InvalidUser => write!(f, "I don't know who that user is..."),
            BirthdayActionError::InvalidDate => {
                write!(f, "That doesn't seem right... make sure it's a valid date.")
            }
            BirthdayActionError::MissingOption => write!(f, "You forgot a required field!"),
            BirthdayActionError::UnknownAction => {
                write!(f, "What the hell are you trying to do...")
            }
            BirthdayActionError::InsufficientPermissions => {
                write!(f, "You need administrator permission to manage birthdays.")
            }
            BirthdayActionError::UserLookupFailed(error) => {
                write!(f, "Failed to look up a birthday member: {error}")
            }
        }
    }
}

/// BIRTHDAY COMMAND
/// /birthday [add|remove] <user> <date>
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
    options: &[ResolvedOption<'_>],
    data: &Mutex<Data>,
    permissions: Option<Permissions>,
    http: &Http,
) -> Result<String, BirthdayActionError> {
    let subcommand = options
        .first()
        .ok_or_else(|| BirthdayActionError::MissingOption)?;
    let ResolvedValue::SubCommand(options) = &subcommand.value else {
        return Err(BirthdayActionError::UnknownAction);
    };

    match subcommand.name {
        "add" => {
            require_admin(permissions)?;
            let user = options
                .iter()
                .find(|option| option.name == "user")
                .ok_or_else(|| BirthdayActionError::MissingOption)?;
            let ResolvedValue::User(user, _) = user.value else {
                return Err(BirthdayActionError::InvalidUser);
            };

            let month = options
                .iter()
                .find(|option| option.name == "month")
                .ok_or_else(|| BirthdayActionError::MissingOption)?;
            let ResolvedValue::Integer(month) = month.value else {
                return Err(BirthdayActionError::InvalidDate);
            };
            let month = month as u32;

            let day = options
                .iter()
                .find(|option| option.name == "day")
                .ok_or_else(|| BirthdayActionError::MissingOption)?;
            let ResolvedValue::Integer(day) = day.value else {
                return Err(BirthdayActionError::InvalidDate);
            };
            let day = day as u32;

            let (month, day) =
                normalized_month_day(month, day).ok_or_else(|| BirthdayActionError::InvalidDate)?;
            let date = format!("{month:02}/{day:02}");
            let user_id = user.id.get();
            match data.lock().await.add_birthday(user_id, month, day) {
                Ok(true) => Ok(format!(
                    "**{}**'s birthday has been added for {}!",
                    user.name, date
                )),
                Ok(false) => Ok(format!("**{}**'s birthday is already set.", user.name)),
                Err(_) => Err(BirthdayActionError::UnknownAction),
            }
        }
        "remove" => {
            require_admin(permissions)?;
            let user = options
                .iter()
                .find(|option| option.name == "user")
                .ok_or_else(|| BirthdayActionError::MissingOption)?;
            let ResolvedValue::User(user, _) = user.value else {
                return Err(BirthdayActionError::InvalidUser);
            };

            let user_id = user.id.get();
            match data.lock().await.remove_birthday(user_id) {
                Ok(true) => Ok(format!("**{}**'s birthday has been removed.", user.name)),
                Ok(false) => Ok(format!("**{}** does not have a birthday set.", user.name)),
                Err(_) => Err(BirthdayActionError::UnknownAction),
            }
        }
        "list" => {
            let birthdays = data.lock().await.get_birthdays();
            list(http, birthdays).await
        }
        _ => return Err(BirthdayActionError::UnknownAction),
    }
}

fn require_admin(permissions: Option<Permissions>) -> Result<(), BirthdayActionError> {
    if permissions.is_some_and(|permissions| permissions.contains(Permissions::ADMINISTRATOR)) {
        Ok(())
    } else {
        Err(BirthdayActionError::InsufficientPermissions)
    }
}

async fn list(http: &Http, birthdays: Vec<(u64, u32, u32)>) -> Result<String, BirthdayActionError> {
    if birthdays.is_empty() {
        return Ok("No birthdays are set.".to_string());
    }

    let mut message = String::new();
    for (user_id, month, day) in birthdays {
        let user = http
            .get_user(UserId::new(user_id))
            .await
            .map_err(|error| BirthdayActionError::UserLookupFailed(error.to_string()))?;
        message.push_str(&format!(
            "**{}**: {:02}/{:02}\n",
            user.display_name(),
            month,
            day
        ));
    }
    Ok(message)
}

fn normalized_month_day(month: u32, day: u32) -> Option<(u32, u32)> {
    chrono::NaiveDate::from_ymd_opt(2000, month, day).map(|date| (date.month(), date.day()))
}
