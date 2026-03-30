//! Keychain domain model and storage helpers.

pub mod model;
pub mod redb_store;
pub mod repository;
pub mod resolver;

pub use model::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
pub use redb_store::RedbKeychainCatalogStore;
pub use repository::KeychainCatalogRepository;
pub use resolver::{
    DerivedSshKeyMaterial, derive_public_key_material_from_private_key,
    derive_public_key_material_from_public_key, resolve_saved_ssh_profile,
};
