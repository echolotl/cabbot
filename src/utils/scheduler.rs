use std::{
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

use chrono::{Datelike, NaiveDate, Utc};
use job_scheduler::{Job, JobScheduler, Schedule};
use serenity::{
    all::{ChannelId, Http, UserId},
    builder::{CreateAllowedMentions, CreateMessage},
};
use tokio::sync::Mutex;

use super::data::Data;

const BIRTHDAY_SCHEDULE: &str = "0 0 22 * * *"; // 22:00 UTC, so scheduled for 00:00 UTC+2

pub struct Scheduler {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<()>,
}

impl Scheduler {
    pub fn stop(self) {
        if self.stop.send(()).is_err() {
            eprintln!("Birthday scheduler stopped unexpectedly");
        }
        if self.thread.join().is_err() {
            eprintln!("Birthday scheduler thread panicked");
        }
    }
}

pub fn start(data: Arc<Mutex<Data>>, http: Arc<Http>, channel: ChannelId) -> Scheduler {
    let schedule: Schedule = BIRTHDAY_SCHEDULE
        .parse()
        .expect("Invalid birthday scheduler cron expression");
    let runtime = tokio::runtime::Handle::current();
    let (stop, receiver) = mpsc::channel();
    let thread = thread::Builder::new()
        .name("birthday-scheduler".into())
        .spawn(move || {
            let mut scheduler = JobScheduler::new();
            scheduler.add(Job::new(schedule, move || {
                let date = (Utc::now() + chrono::Duration::hours(2)).date_naive();
                runtime.spawn(announce_birthdays(
                    Arc::clone(&data),
                    Arc::clone(&http),
                    channel,
                    date,
                ));
            }));
            loop {
                scheduler.tick();
                match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
        .expect("Failed to start birthday scheduler");
    Scheduler { stop, thread }
}

async fn announce_birthdays(
    data: Arc<Mutex<Data>>,
    http: Arc<Http>,
    channel: ChannelId,
    date: NaiveDate,
) {
    let birthdays = data.lock().await.get_birthdays();
    for (user_id, month, day) in birthdays {
        if !is_birthday(date, month, day) {
            continue;
        }
        let user = UserId::new(user_id);
        let message = CreateMessage::new()
            .content(format!("It's <@{user_id}>'s birthday!"))
            .allowed_mentions(CreateAllowedMentions::new().users([user]));
        if let Err(error) = channel.send_message(&http, message).await {
            eprintln!("Failed to send birthday announcement for user {user_id}: {error}");
        }
    }
}

fn is_birthday(date: NaiveDate, month: u32, day: u32) -> bool {
    (date.month(), date.day()) == (month, day)
        || (month, day) == (2, 29)
            && (date.month(), date.day()) == (2, 28)
            && NaiveDate::from_ymd_opt(date.year(), 2, 29).is_none()
}
