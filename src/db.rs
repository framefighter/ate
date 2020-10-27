use nanoid::nanoid;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use std::fmt;

use crate::meal::Meal;

#[derive(Debug)]
pub enum DBKeys {
    State,
    MealsChat,
    Whitelist,
}

impl fmt::Display for DBKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct StoreHandler {
    pub db: pickledb::PickleDb,
    pub meal_db: pickledb::PickleDb,
}

impl StoreHandler {
    pub fn new(do_backup: bool) -> Self {
        let mut sh = StoreHandler {
            db: Self::create(DBKeys::State),
            meal_db: Self::create(DBKeys::MealsChat),
        };
        sh.create_list(DBKeys::Whitelist);
        if do_backup {
            sh.backup(DBKeys::State);
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
        let mut db_backup =
            Self::create_json(format!("database/{}_backup_{}.json", key, nanoid!()), false);
        match db_backup.lcreate(&key.to_string()) {
            Ok(_) => {
                log::info!("Backing up {}!", key);
                for item in self.db.liter(&key.to_string()) {
                    match item.get_item::<Meal>() {
                        Some(meal) => {
                            log::info!("Backing up {}: {}", key, meal.name.clone());
                            db_backup.ladd(&key.to_string(), &meal);
                        }
                        None => {}
                    }
                }
            }
            Err(err) => log::warn!("{}", err),
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
                PickleDb::new(path, PickleDbDumpPolicy::AutoDump, SerializationMethod::Json)
            }
        }
    }

    fn create_json(path: String, load: bool) -> PickleDb {
        let loaded_db = PickleDb::load(
            path.clone(),
            PickleDbDumpPolicy::AutoDump,
            SerializationMethod::Json,
        );
        if loaded_db.is_ok() && load {
            log::info!("Found existing {} database!", path.clone());
            loaded_db.unwrap()
        } else {
            log::info!("Creating new {} database!", path.clone(),);
            PickleDb::new(
                path,
                PickleDbDumpPolicy::AutoDump,
                SerializationMethod::Json,
            )
        }
    }
}
