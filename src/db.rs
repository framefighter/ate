use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use std::fmt;

#[derive(Debug)]
pub enum DBKeys {
    Meals,
    Keyboards,
    Whitelist,
}

impl fmt::Display for DBKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Into<String> for DBKeys {
    fn into(self) -> String {
        format!("{:?}", self)
    }
}

pub struct StoreHandler {
    pub db: pickledb::PickleDb,
}

impl Default for StoreHandler {
    fn default() -> Self {
        let loaded_db = PickleDb::load(
            "meals.db",
            PickleDbDumpPolicy::AutoDump,
            SerializationMethod::Bin,
        );
        let mut db = if loaded_db.is_ok() {
            loaded_db.unwrap()
        } else {
            PickleDb::new(
                "meals.db",
                PickleDbDumpPolicy::AutoDump,
                SerializationMethod::Bin,
            )
        };
        if !db.lexists(&DBKeys::Meals.to_string()) {
            if let Err(err) = db.lcreate(&DBKeys::Meals.to_string()) {
                log::warn!("{}", err);
            }
        }
        if !db.lexists(&DBKeys::Whitelist.to_string()) {
            if let Err(err) = db.lcreate(&DBKeys::Whitelist.to_string()) {
                log::warn!("{}", err);
            }
        }
        if !db.lexists(&DBKeys::Keyboards.to_string()) {
            if let Err(err) = db.lcreate(&DBKeys::Keyboards.to_string()) {
                log::warn!("{}", err);
            }
        }
        StoreHandler { db }
    }
}
