use serenity::all::{Context, User, UserId};

use crate::commands::CabbotCommandError;

const MAX_LOOKUPS: usize = 10;

pub async fn user_lookup(
    ctx: &Context,
    user_ids: Vec<u64>,
) -> Result<Vec<User>, CabbotCommandError> {
    let mut pending = user_ids.into_iter().enumerate();
    let mut joinset = tokio::task::JoinSet::new();
    let mut users = Vec::new();

    for _ in 0..MAX_LOOKUPS {
        if let Some((index, user_id)) = pending.next() {
            let context = ctx.clone();
            joinset.spawn(async move {
                match UserId::new(user_id).to_user(context).await {
                    Ok(user) => Ok((index, user)),
                    Err(e) => {
                        log::error!("Failed to look up birthday user {user_id}: {e}");
                        Err(CabbotCommandError::LookupFailed("user".to_string()))
                    }
                }
            });
        }
    }

    while let Some(result) = joinset.join_next().await {
        match result {
            Ok(Ok((index, user))) => users.push((index, user)),
            Ok(Err(error)) => return Err(error),
            Err(error) => {
                log::error!("User lookup task failed: {error}");
                return Err(CabbotCommandError::LookupFailed("user".to_string()));
            }
        }

        if let Some((index, user_id)) = pending.next() {
            let context = ctx.clone();
            joinset.spawn(async move {
                match UserId::new(user_id).to_user(context).await {
                    Ok(user) => Ok((index, user)),
                    Err(e) => {
                        log::error!("Failed to look up user {user_id}: {e}");
                        Err(CabbotCommandError::LookupFailed("user".to_string()))
                    }
                }
            });
        }
    }

    users.sort_by_key(|(index, _)| *index);
    Ok(users.into_iter().map(|(_, user)| user).collect())
}
