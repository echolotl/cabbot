use serde::{Deserialize, Serialize};
use std::io;

static DATA_FILE_PATH: &str = "data/data.json";
static CURRENT_VERSION: &str = "1.0.0";

/// A user's birthday.
#[derive(Clone, Serialize, Deserialize)]
pub struct Birthday {
    pub user_id: u64,
    pub month: u32,
    pub day: u32,
}

/// The persistent data structure for the bot.
#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    version: String,
    birthdays: Vec<Birthday>,
}

impl Data {
    pub fn get() -> io::Result<Self> {
        match std::fs::File::open(DATA_FILE_PATH) {
            Ok(file) => {
                let mut data: Data = serde_json::from_reader(file).map_err(io::Error::other)?;
                data.migrate()?;
                Ok(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Data {
                version: CURRENT_VERSION.to_string(),
                birthdays: Vec::new(),
            }),
            Err(error) => Err(error),
        }
    }

    pub fn add_birthday(&mut self, user_id: u64, month: u32, day: u32) -> io::Result<bool> {
        if self
            .birthdays
            .iter()
            .any(|birthday| birthday.user_id == user_id)
        {
            return Ok(false);
        }

        let mut updated = self.clone();
        updated.birthdays.push(Birthday {
            user_id,
            month,
            day,
        });
        updated.save()?;
        *self = updated;
        Ok(true)
    }

    pub fn remove_birthday(&mut self, user_id: u64) -> io::Result<bool> {
        if !self
            .birthdays
            .iter()
            .any(|birthday| birthday.user_id == user_id)
        {
            return Ok(false);
        }

        let mut updated = self.clone();
        updated
            .birthdays
            .retain(|birthday| birthday.user_id != user_id);
        updated.save()?;
        *self = updated;
        Ok(true)
    }

    pub fn get_birthdays(&self) -> Vec<(u64, u32, u32)> {
        self.birthdays
            .iter()
            .map(|birthday| (birthday.user_id, birthday.month, birthday.day))
            .collect()
    }

    fn save(&self) -> io::Result<()> {
        std::fs::create_dir_all("data")?;
        let file = std::fs::File::create(DATA_FILE_PATH)?;
        serde_json::to_writer_pretty(file, self).map_err(io::Error::other)
    }

    fn migrate(&mut self) -> io::Result<()> {
        if self.version != CURRENT_VERSION {
            self.version = CURRENT_VERSION.to_string();
            self.save()?;
        }
        Ok(())
    }
}
