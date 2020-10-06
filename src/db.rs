use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use std::fmt;

#[derive(Debug)]
pub enum DBKeys {
    Meals,
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

impl StoreHandler {
    pub fn new() -> Self {
        let loaded_db = PickleDb::load(
            "meals.db",
            PickleDbDumpPolicy::AutoDump,
            SerializationMethod::Json,
        );
        let mut db = if loaded_db.is_ok() {
            loaded_db.unwrap()
        } else {
            PickleDb::new(
                "meals.db",
                PickleDbDumpPolicy::AutoDump,
                SerializationMethod::Json,
            )
        };
        let key: String = DBKeys::Meals.into();
        if !db.lexists(&key) {
            let _ = db.lcreate(&key);
        }
        StoreHandler { db }
    }

    pub fn save<T: serde::ser::Serialize>(&mut self, key: DBKeys, value: T) {
        self.db.set(&*key.to_string(), &value).ok();
        self.db.dump().ok();
    }

    pub fn load<T: serde::de::DeserializeOwned>(&self, key: DBKeys, default_val: T) -> T {
        self.db.get::<T>(&*key.to_string()).unwrap_or(default_val)
    }

    pub fn clear(&mut self) {
        self.db.get_all().iter().for_each(|k| {
            self.db.rem(k).ok();
        })
    }
}
