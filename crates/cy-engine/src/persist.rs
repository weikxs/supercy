//! Settings and the device tag, stored as sealed meta records.

use cy_model::{DeviceTag, ItemId};
use cy_store::Store;

use crate::{EngineError, Settings};

/// Meta record holding the settings.
pub(crate) const SETTINGS_KEY: &str = "settings";
/// Meta record holding this device's tag.
const DEVICE_KEY: &str = "device-tag";

pub(crate) fn load_or_create_device(store: &mut Store) -> Result<DeviceTag, EngineError> {
    if let Some(stored) = store.read_meta(DEVICE_KEY)? {
        let bytes =
            <[u8; 8]>::try_from(stored.expose()).map_err(|_| cy_store::StoreError::Corrupt)?;
        return Ok(DeviceTag(bytes));
    }
    let random = ItemId::random().map_err(|_| EngineError::Random)?;
    let mut tag = [0u8; 8];
    tag.copy_from_slice(&random.as_bytes()[..8]);
    store.write_meta(DEVICE_KEY, &tag)?;
    Ok(DeviceTag(tag))
}

/// Stored settings, or the defaults when there are none. Settings that no longer validate (for
/// example after ranges changed) fall back to the defaults rather than blocking the history.
pub(crate) fn load_settings(store: &Store) -> Result<Settings, EngineError> {
    let Some(stored) = store.read_meta(SETTINGS_KEY)? else {
        return Ok(Settings::default());
    };
    let settings: Settings = ciborium::from_reader(stored.expose()).unwrap_or_default();
    Ok(if settings.validate().is_ok() {
        settings
    } else {
        Settings::default()
    })
}
