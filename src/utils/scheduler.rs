use std::{
    collections::HashMap,
    sync::{Arc, RwLock, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::utils::data::Birthday;
use chrono::{Datelike, NaiveDate, Timelike, Utc};
use log::{error, info, warn};
use serenity::{
    all::{ChannelId, Http, UserId},
    builder::{CreateAllowedMentions, CreateMessage},
};

use super::data::{Data, Settings, read_lock};

/// Scheduler for announcing birthdays at a specified time.
/// It runs in a seperate thread.
pub struct Scheduler {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<()>,
}

impl Scheduler {
    pub fn stop(self) {
        if self.stop.send(()).is_err() {
            warn!("Birthday scheduler stop signal could not be delivered");
        }
        if self.thread.join().is_err() {
            error!("Birthday scheduler thread panicked");
        } else {
            info!("Birthday scheduler stopped");
        }
    }
}

pub fn start(
    data: Arc<RwLock<HashMap<u64, Data>>>,
    settings: Arc<RwLock<HashMap<u64, Settings>>>,
    http: Arc<Http>,
) -> Scheduler {
    let runtime = tokio::runtime::Handle::current();
    let (stop, receiver) = mpsc::channel();
    let thread = thread::Builder::new()
        .name("birthday-scheduler".into())
        .spawn(move || {
            // We lock into the start of the next hour before beginning the loop.
            let now = Utc::now();
            let seconds_into_hour = u64::from(now.minute() * 60 + now.second());
            let initial_delay = if seconds_into_hour == 0 && now.nanosecond() == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs(3600 - seconds_into_hour)
                    - Duration::from_nanos(u64::from(now.nanosecond()))
            };
            info!("Birthday scheduler started! First check in {initial_delay:?}");
            thread::sleep(initial_delay);

            loop {
                // Once the loop starts, we check once per hour, just because really long sleeps can drift over time.
                runtime.block_on(async {
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
                });
                let now = Utc::now();
                let seconds_into_hour = u64::from(now.minute() * 60 + now.second());
                let delay = Duration::from_secs(3600 - seconds_into_hour)
                    - Duration::from_nanos(u64::from(now.nanosecond()));
                match receiver.recv_timeout(delay) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
        .unwrap_or_else(|error| {
            log::error!("Failed to start birthday scheduler thread: {error}");
            panic!("Failed to start birthday scheduler");
        });
    Scheduler { stop, thread }
}

async fn check_and_announce_birthdays(
    data: &Data,
    http: Arc<Http>,
    channel: ChannelId,
    date: NaiveDate,
) {
    // Checks if the given date matches the birthday specified by month and day.
    // feb 29 birthdays are treated as feb 28 on non-leap years
    fn is_birthday(date: NaiveDate, month: u32, day: u32) -> bool {
        (date.month(), date.day()) == (month, day)
            || (month, day) == (2, 29)
                && (date.month(), date.day()) == (2, 28)
                && NaiveDate::from_ymd_opt(date.year(), 2, 29).is_none()
    }
    let birthdays = data.birthdays.clone();
    for Birthday {
        user_id,
        month,
        day,
    } in birthdays
    {
        if !is_birthday(date, month, day) {
            continue;
        }
        let user = UserId::new(user_id);
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
