use serde::{Deserialize, Serialize};
use std::io;

static DATA_FILE_PATH: &str = "data.json";
static SETTINGS_FILE_PATH: &str = "settings.json";
static DATA_DIR: &str = "data";
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
    pub birthdays: Vec<Birthday>,
}

// The persistent settings structure for the bot.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    version: String,
    pub birthday_channel: u64,
}

impl Data {
    pub fn get() -> io::Result<Self> {
        match std::fs::File::open(format!("{}/{}", DATA_DIR, DATA_FILE_PATH)) {
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

    pub fn save(&self) -> io::Result<()> {
        std::fs::create_dir_all(DATA_DIR)?;
        let file = std::fs::File::create(format!("{}/{}", DATA_DIR, DATA_FILE_PATH))?;
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

impl Settings {
    pub fn get() -> io::Result<Self> {
        match std::fs::File::open(format!("{}/{}", DATA_DIR, SETTINGS_FILE_PATH)) {
            Ok(file) => {
                let mut settings: Settings =
                    serde_json::from_reader(file).map_err(io::Error::other)?;
                settings.migrate()?;
                Ok(settings)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings {
                version: CURRENT_VERSION.to_string(),
                birthday_channel: 0,
            }),
            Err(error) => Err(error),
        }
    }
    pub fn save(&self) -> io::Result<()> {
        std::fs::create_dir_all(DATA_DIR)?;
        let file = std::fs::File::create(format!("{}/{}", DATA_DIR, SETTINGS_FILE_PATH))?;
        serde_json::to_writer_pretty(file, self).map_err(io::Error::other)
    }

    fn migrate(&mut self) -> io::Result<()> {
        let mut changed = false;
        if self.version != CURRENT_VERSION {
            self.version = CURRENT_VERSION.to_string();
            changed = true;
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }
}
