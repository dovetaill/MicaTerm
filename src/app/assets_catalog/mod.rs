//! Persisted asset catalog domain and mapping helpers.

pub mod mapper;
pub mod model;
pub mod redb_store;
pub mod repository;

pub use mapper::{
    asset_tree_to_catalog, asset_tree_to_vault_catalog, asset_trees_to_catalog,
    catalog_to_asset_tree, catalog_to_asset_trees, vault_catalog_to_asset_tree,
};
pub use model::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetCatalog, PersistedAssetDomain,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload,
    PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec, PersistedSnippetSpec,
    PersistedSshConnectionSpec,
};
pub use redb_store::{
    ASSET_RECORDS_TABLE, METADATA_ROOT_IDS_KEY, METADATA_SCHEMA_VERSION_KEY, METADATA_TABLE,
    RedbAssetCatalogStore,
};
pub use repository::AssetCatalogRepository;
