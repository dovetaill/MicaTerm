//! Repository boundary for loading and saving the persisted asset catalog.

use anyhow::Result;

use crate::app::assets_catalog::PersistedAssetCatalog;

pub trait AssetCatalogRepository {
    fn load(&self) -> Result<PersistedAssetCatalog>;
    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()>;
}
