use pickledb::error::Error;
use serde::{de::DeserializeOwned, Serialize};

use crate::db::{DBKeys, StoreHandler};
use crate::{Config, StateLock};

pub trait HasId {
    fn id(&self) -> String;
    fn chat_id(&self) -> i64;
    fn save(&self, state: &StateLock) -> Self;
}

pub struct State {
    store_handler: StoreHandler,
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        let store_handler = StoreHandler::new(config.backup);
        Self {
            store_handler,
            config,
        }
    }

    pub fn add<T: Serialize + HasId + Clone>(&mut self, entry: &T) -> Result<T, Error> {
        match self.store_handler.db.set::<T>(&entry.id(), entry) {
            Ok(_) => Ok(entry.clone()),
            Err(err) => Err(err),
        }
    }

    pub fn get<T: DeserializeOwned>(&self, id: &String) -> Option<T> {
        self.store_handler.db.get::<T>(id)
    }

    pub fn all<T: DeserializeOwned + HasId>(&self) -> Vec<T> {
        self.store_handler
            .db
            .get_all()
            .iter()
            .filter_map(|key| self.store_handler.db.get::<T>(&key))
            .collect()
    }

    pub fn all_chat<T: DeserializeOwned + HasId>(&self, chat_id: i64) -> Vec<T> {
        self.store_handler
            .db
            .get_all()
            .iter()
            .filter_map(|key| self.store_handler.db.get::<T>(&key))
            .filter(|entry| entry.chat_id() == chat_id)
            .collect()
    }

    pub fn find<F, T: DeserializeOwned + HasId>(&self, chat_id: i64, finder: F) -> Option<T>
    where
        F: Fn(&T) -> bool,
    {
        self.all_chat(chat_id).into_iter().find(finder)
    }

    pub fn find_all<F, T: DeserializeOwned + HasId>(&self, finder: F) -> Option<T>
    where
        F: Fn(&T) -> bool,
    {
        self.all().into_iter().find(finder)
    }

    pub fn filter<F, T: DeserializeOwned + HasId>(&self, chat_id: i64, finder: F) -> Vec<T>
    where
        F: Fn(&T) -> bool,
    {
        self.all_chat(chat_id).into_iter().filter(finder).collect()
    }

    pub fn modify<F, T: DeserializeOwned + Serialize + Clone>(
        &mut self,
        id: &String,
        modifier: F,
    ) -> Result<T, String>
    where
        F: Fn(T) -> T,
    {
        match self.store_handler.db.get::<T>(id) {
            Some(entry) => {
                let modified = modifier(entry);
                match self.store_handler.db.set::<T>(id, &modified) {
                    Ok(_) => Ok(modified),
                    Err(_) => Err(format!("Failed to store modified entry!")),
                }
            }
            None => Err(format!("No entry to modify found!")),
        }
    }

    pub fn remove(&mut self, id: &String) -> Result<bool, Error> {
        self.store_handler.db.rem(id)
    }

    pub fn whitelist_user(&mut self, username: String) {
        self.store_handler
            .db
            .ladd(&DBKeys::Whitelist.to_string(), &username);
        log::info!("Whitelisting User: {}", username);
    }

    pub fn get_whitelisted_users(&self) -> Vec<String> {
        self.store_handler
            .db
            .liter(&DBKeys::Whitelist.to_string())
            .filter_map(|item| item.get_item::<String>())
            .collect()
    }
}
