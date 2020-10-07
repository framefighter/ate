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
}
