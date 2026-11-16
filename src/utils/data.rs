use serde::{Deserialize, Serialize};
use serenity::all::prelude::TypeMapKey;
use std::io;

static DATA_FILE_PATH: &str = "data.json";
static SETTINGS_FILE_PATH: &str = "settings.json";
static DATA_DIR: &str = "data";
static DATA_VERSION: &str = "1.0.1";
static SETTINGS_VERSION: &str = "1.0.0";

/// A user's birthday.
#[derive(Clone, Serialize, Deserialize)]
pub struct Birthday {
    pub user_id: u64,
    #[serde(rename = "m")]
    pub month: u32,
    #[serde(rename = "d")]
    pub day: u32,
}

/// The persistent data structure for the bot.
#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    version: String,
    pub birthdays: Vec<Birthday>,
}

impl TypeMapKey for Data {
    type Value = std::sync::Arc<std::sync::RwLock<Data>>;
}

// The persistent settings structure for the bot.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    version: String,
    pub birthday_channel: u64,
}

impl TypeMapKey for Settings {
    type Value = std::sync::Arc<std::sync::RwLock<Settings>>;
}

impl Data {
    pub fn get() -> io::Result<Self> {
        match std::fs::File::open(format!("{}/{}", DATA_DIR, DATA_FILE_PATH)) {
            Ok(file) => {
                let mut value: serde_json::Value =
                    serde_json::from_reader(file).map_err(io::Error::other)?;
                let changed = Self::migrate(&mut value);
                let data: Data = serde_json::from_value(value).map_err(io::Error::other)?;
                if changed {
                    data.save()?;
                }
                Ok(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Data {
                version: DATA_VERSION.to_string(),
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

    fn migrate(value: &mut serde_json::Value) -> bool {
        let mut changed = false;

        if migrate_version(value, DATA_VERSION) {
            changed = true;
        } else {
            // If the version isn't different, then we don't have to migrate anything.
            return changed;
        }

        // v1.0.1: Rename "month" to "m" and "day" to "d" in birthdays
        if let Some(birthdays) = value
            .as_object_mut()
            .and_then(|object| object.get_mut("birthdays"))
            .and_then(serde_json::Value::as_array_mut)
        {
            for birthday in birthdays {
                if let Some(object) = birthday.as_object_mut() {
                    for (old_key, new_key) in [("month", "m"), ("day", "d")] {
                        if let Some(old_value) = object.remove(old_key) {
                            object.entry(new_key).or_insert(old_value);
                            changed = true;
                        }
                    }
                }
            }
        }

        changed
    }
}

impl Settings {
    pub fn get() -> io::Result<Self> {
        match std::fs::File::open(format!("{}/{}", DATA_DIR, SETTINGS_FILE_PATH)) {
            Ok(file) => {
                let mut value: serde_json::Value =
                    serde_json::from_reader(file).map_err(io::Error::other)?;
                let changed = Self::migrate(&mut value);
                let settings: Settings = serde_json::from_value(value).map_err(io::Error::other)?;
                if changed {
                    settings.save()?;
                }
                Ok(settings)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings {
                version: SETTINGS_VERSION.to_string(),
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

    fn migrate(value: &mut serde_json::Value) -> bool {
        let mut changed = false;
        
        if migrate_version(value, DATA_VERSION) {
            changed = true;
        } else {
            // If the version isn't different, then we don't have to migrate anything.
            return changed;
        }

        changed
    }

}

fn migrate_version(value: &mut serde_json::Value, version: &str) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let Some(v) = object.get("version").and_then(serde_json::Value::as_str) else {
        return false;
    };
    if v == version {
        return false;
    }

    object.insert(
        "version".to_string(),
        serde_json::Value::String(version.to_string()),
    );
    true
}
