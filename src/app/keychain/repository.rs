//! Repository boundary for loading and saving the persisted keychain catalog.

use anyhow::Result;

use crate::app::keychain::model::KeychainCatalog;

pub trait KeychainCatalogRepository {
    fn load(&self) -> Result<KeychainCatalog>;
    fn save(&self, catalog: &KeychainCatalog) -> Result<()>;
}
