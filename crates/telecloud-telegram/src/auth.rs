use keyring::Entry;
use telecloud_core::{Result, TeleCloudError};

const SERVICE_NAME: &str = "TeleCloud";

pub struct TelegramAuthManager;

impl TelegramAuthManager {
    pub fn save_credential(key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, key).map_err(|e| TeleCloudError::Auth {
            operation: "init_keyring_entry".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        entry.set_password(value).map_err(|e| TeleCloudError::Auth {
            operation: "save_credential".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        Ok(())
    }

    pub fn get_credential(key: &str) -> Result<Option<String>> {
        let entry = Entry::new(SERVICE_NAME, key).map_err(|e| TeleCloudError::Auth {
            operation: "init_keyring_entry".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        match entry.get_password() {
            Ok(pwd) => Ok(Some(pwd)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(TeleCloudError::Auth {
                operation: "get_credential".to_string(),
                message: e.to_string(),
                recoverable: false,
            }),
        }
    }

    pub fn delete_credential(key: &str) -> Result<()> {
        let entry = Entry::new(SERVICE_NAME, key).map_err(|e| TeleCloudError::Auth {
            operation: "init_keyring_entry".to_string(),
            message: e.to_string(),
            recoverable: false,
        })?;
        match entry.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(TeleCloudError::Auth {
                operation: "delete_credential".to_string(),
                message: e.to_string(),
                recoverable: false,
            }),
        }
    }
}
