use chrono::offset::Local;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use std::fmt;
use std::fs;

#[derive(Debug)]
pub enum DBKeys {
    Store,
    Whitelist,
}

impl fmt::Display for DBKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct StoreHandler {
    pub db: pickledb::PickleDb,
}

impl StoreHandler {
    pub fn new(do_backup: bool) -> Self {
        let mut sh = StoreHandler {
            db: Self::create(DBKeys::Store),
        };
        sh.create_list(DBKeys::Whitelist);
        if do_backup {
            sh.backup(DBKeys::Store);
        }
        sh
    }

    fn create_list(&mut self, key: DBKeys) {
        if !self.db.lexists(&key.to_string()) {
            match self.db.lcreate(&key.to_string()) {
                Ok(_) => log::info!("Created new list: {}", key),
                Err(err) => log::warn!("{}", err),
            }
        } else {
            log::info!("Found existing list: {}", key);
        }
    }

    fn backup(&self, key: DBKeys) {
        let source = format!("database/{}.json", key.to_string().to_lowercase());
        let target = format!(
            "database/{}_backup_{}.json",
            key.to_string().to_lowercase(),
            Local::now().format("%d-%m-%Y_%H-%M")
        );
        match fs::copy(source, target) {
            Ok(_) => log::info!("Backed up database!"),
            Err(err) => log::warn!("Error backing up database!: {}", err),
        }
    }

    fn create(key: DBKeys) -> PickleDb {
        let path = format!("database/{}.json", key.to_string().to_lowercase());
        match PickleDb::load(
            path.clone(),
            PickleDbDumpPolicy::AutoDump,
            SerializationMethod::Json,
        ) {
            Ok(db) => {
                log::info!("Found existing {} database!", path);
                db
            }
            Err(err) => {
                log::warn!("{}", err);
                log::info!("Creating new {} database!", path);
                PickleDb::new(
                    path,
                    PickleDbDumpPolicy::AutoDump,
                    SerializationMethod::Json,
                )
            }
        }
    }
}
