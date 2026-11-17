use log::{debug, info};
use serde::{Deserialize, Serialize};
use serenity::all::prelude::TypeMapKey;
use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

static DATA_FILE_PATH: &str = "data.json";
static SETTINGS_FILE_PATH: &str = "settings.json";
static DATA_DIR: &str = "data";

// x.y.z versioning
// z: Update when making backward-compatible or easily fixable changes
// y: Update when adding a new field or any other backward-incompatible change
// x: Update when making major changes 
static DATA_VERSION: &str = "1.0.1";
static SETTINGS_VERSION: &str = "1.0.0";

/// A user's birthday.
#[derive(Clone, Serialize, Deserialize, Hash, Default)]
pub struct Birthday {
    pub user_id: u64,
    #[serde(rename = "m")]
    pub month: u32,
    #[serde(rename = "d")]
    pub day: u32,
}

impl PartialEq for Birthday {
    fn eq(&self, other: &Self) -> bool {
        self.user_id == other.user_id && self.month == other.month && self.day == other.day
    }
}
impl Eq for Birthday {}

/// The persistent data structure for the bot.
#[derive(Clone, Serialize, Deserialize, Hash, Default)]
pub struct Data {
    version: String,
    pub birthdays: Vec<Birthday>,
}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.birthdays == other.birthdays
    }
}
impl Eq for Data {}

impl TypeMapKey for Data {
    type Value = Arc<RwLock<HashMap<u64, Data>>>;
}

// The persistent settings structure for the bot.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    version: String,
    pub birthday_channel: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION.to_string(),
            birthday_channel: 0,
        }
    }
}

impl TypeMapKey for Settings {
    type Value = Arc<RwLock<HashMap<u64, Settings>>>;
}

impl Data {
    pub fn get(guild_id: u64) -> io::Result<Self> {
        let path: PathBuf = [DATA_DIR, &guild_id.to_string(), DATA_FILE_PATH]
            .iter()
            .collect();
        match std::fs::File::open(&path) {
            Ok(file) => {
                let mut value: serde_json::Value =
                    serde_json::from_reader(file).map_err(io::Error::other)?;
                let changed = Self::migrate(&mut value);
                let data: Data = serde_json::from_value(value).map_err(io::Error::other)?;
                if changed {
                    info!("Migrated birthday data for guild {guild_id}");
                    data.save(guild_id)?;
                }
                debug!(
                    "Loaded {} birthdays for guild {guild_id}",
                    data.birthdays.len()
                );
                Ok(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                debug!("No birthday data found for guild {guild_id}! Defaulting...");
                let data = Self::default();
                data.save(guild_id)?;
                Ok(data)
            }
            Err(error) => Err(error),
        }
    }

    /// Retrieve all data for all guilds as a HashMap keyed by guild ID.
    pub fn get_all() -> io::Result<HashMap<u64, Self>> {
        let mut all_data = HashMap::new();
        for guild_id in get_all_guilds()? {
            all_data.insert(guild_id, Self::get(guild_id)?);
        }
        Ok(all_data)
    }

    pub fn save(&self, guild_id: u64) -> io::Result<()> {
        let path: PathBuf = [DATA_DIR, &guild_id.to_string()].iter().collect();
        std::fs::create_dir_all(&path)?;
        let file = std::fs::File::create(path.join(DATA_FILE_PATH))?;
        serde_json::to_writer_pretty(file, self).map_err(io::Error::other)?;
        debug!(
            "Saved {} birthdays for guild {guild_id}",
            self.birthdays.len()
        );
        Ok(())
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
    pub fn get(guild_id: u64) -> io::Result<Self> {
        let path: PathBuf = [DATA_DIR, &guild_id.to_string(), SETTINGS_FILE_PATH]
            .iter()
            .collect();
        match std::fs::File::open(&path) {
            Ok(file) => {
                let mut value: serde_json::Value =
                    serde_json::from_reader(file).map_err(io::Error::other)?;
                let changed = Self::migrate(&mut value);
                let settings: Settings = serde_json::from_value(value).map_err(io::Error::other)?;
                if changed {
                    info!("Migrated settings for guild {guild_id}");
                    settings.save(guild_id)?;
                }
                debug!("Loaded settings for guild {guild_id}");
                Ok(settings)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                debug!("No settings found for guild {guild_id}! Defaulting...");
                let settings = Self::default();
                settings.save(guild_id)?;
                Ok(settings)
            }
            Err(error) => Err(error),
        }
    }

    pub fn get_all() -> io::Result<HashMap<u64, Self>> {
        let mut all_data = HashMap::new();
        for guild_id in get_all_guilds()? {
            all_data.insert(guild_id, Self::get(guild_id)?);
        }
        Ok(all_data)
    }

    pub fn save(&self, guild_id: u64) -> io::Result<()> {
        let path: PathBuf = [DATA_DIR, &guild_id.to_string()].iter().collect();
        std::fs::create_dir_all(&path)?;
        let file = std::fs::File::create(path.join(SETTINGS_FILE_PATH))?;
        serde_json::to_writer_pretty(file, self).map_err(io::Error::other)?;
        debug!("Saved settings for guild {guild_id}");
        Ok(())
    }

    fn migrate(value: &mut serde_json::Value) -> bool {
        let mut changed = false;

        if migrate_version(value, SETTINGS_VERSION) {
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

fn get_all_guilds() -> io::Result<Vec<u64>> {
    let mut guilds = Vec::new();
    let entries = match std::fs::read_dir(DATA_DIR) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(guilds),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if let Some(guild_id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u64>().ok())
        {
            guilds.push(guild_id);
        }
    }
    Ok(guilds)
}
