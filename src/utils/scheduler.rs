use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::utils::data::Birthday;
use chrono::{Datelike, NaiveDate, Timelike, Utc};
use log::{error, info, warn};
use serenity::{
    all::{ChannelId, Http, UserId},
    builder::{CreateAllowedMentions, CreateMessage},
};
use tokio::{
    sync::oneshot,
    task::JoinHandle,
    time::{Instant, MissedTickBehavior, interval_at},
};

use super::data::{Data, Settings, read_lock};

/// Scheduler for announcing birthdays at the start of each UTC day.
pub struct Scheduler {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl Scheduler {
    pub async fn stop(self) {
        if self.stop.send(()).is_err() {
            warn!("The scheduler's stop signal could not be delivered");
        }
        if self.task.await.is_err() {
            error!("Scheduler task panicked");
        } else {
            info!("Scheduler stopped");
        }
    }
}

pub fn start(
    data: Arc<RwLock<HashMap<u64, Data>>>,
    settings: Arc<RwLock<HashMap<u64, Settings>>>,
    http: Arc<Http>,
) -> Scheduler {
    let (stop, mut stopped) = oneshot::channel();
    let task = tokio::spawn(async move {
        let initial_delay = duration_until_next_hour();
        info!("Scheduler started! First check in {initial_delay:?}");
        let mut ticker = interval_at(Instant::now() + initial_delay, Duration::from_secs(3600));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = &mut stopped => break,
                _ = ticker.tick() => {
                    let check = async {
                        let now = Utc::now();
                        // Only announce birthdays at the start of a new UTC day.
                        let Some(date) = (now.hour() == 0).then(|| now.date_naive()) else {
                            return;
                        };
                        info!("Checking birthday announcements for {date}");
                        let guild_settings: Vec<_> = read_lock(&settings)
                            .iter()
                            .map(|(guild_id, settings)| (*guild_id, settings.clone()))
                            .collect();
                        for (
                            guild_id,
                            Settings {
                                birthday_channel, ..
                            },
                        ) in guild_settings
                        {
                            if birthday_channel == 0 {
                                continue;
                            }
                            let guild_data = read_lock(&data).get(&guild_id).cloned();
                            if let Some(guild_data) = guild_data {
                                check_and_announce_birthdays(
                                    &guild_data,
                                    http.clone(),
                                    ChannelId::new(birthday_channel),
                                    date,
                                )
                                .await;
                            }
                        }
                    };
                    tokio::select! {
                        _ = &mut stopped => break,
                        _ = check => {}
                    }
                }
            }
        }
    });
    Scheduler { stop, task }
}

fn duration_until_next_hour() -> Duration {
    let now = Utc::now();
    let seconds_into_hour = u64::from(now.minute() * 60 + now.second());
    if seconds_into_hour == 0 && now.nanosecond() == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs(3600 - seconds_into_hour)
            - Duration::from_nanos(u64::from(now.nanosecond()))
    }
}

async fn check_and_announce_birthdays(
    data: &Data,
    http: Arc<Http>,
    channel: ChannelId,
    date: NaiveDate,
) {
    // Feb. 29 birthdays are treated as Feb. 28 in non-leap years.
    fn is_birthday(date: NaiveDate, month: u32, day: u32) -> bool {
        (date.month(), date.day()) == (month, day)
            || (month, day) == (2, 29)
                && (date.month(), date.day()) == (2, 28)
                && NaiveDate::from_ymd_opt(date.year(), 2, 29).is_none()
    }
    for Birthday {
        user_id,
        month,
        day,
    } in &data.birthdays
    {
        if !is_birthday(date, *month, *day) {
            continue;
        }
        let user = UserId::new(*user_id);
        let message = CreateMessage::new()
            .content(format!("It's <@{user_id}>'s birthday!!!! 🎉🎉🎉"))
            .allowed_mentions(CreateAllowedMentions::new().users([user]));
        if let Err(error) = channel.send_message(&http, message).await {
            error!("Failed to send birthday announcement for user {user_id}: {error}");
        } else {
            info!("Sent birthday announcement for user {user_id}");
        }
    }
}
