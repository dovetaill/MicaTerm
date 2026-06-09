use std::sync::atomic::{AtomicUsize, Ordering};

use fontdb::Database;

static SYSTEM_FONT_DATABASE_LOAD_CALLS: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn load_system_font_database() -> Database {
    SYSTEM_FONT_DATABASE_LOAD_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut database = Database::new();
    database.load_system_fonts();
    database
}

pub(crate) fn system_font_database_load_call_count() -> usize {
    SYSTEM_FONT_DATABASE_LOAD_CALLS.load(Ordering::Relaxed)
}

pub(crate) fn reset_system_font_database_load_call_count() {
    SYSTEM_FONT_DATABASE_LOAD_CALLS.store(0, Ordering::Relaxed);
}
