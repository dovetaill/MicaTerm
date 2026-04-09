use fontdb::Database;

pub(crate) fn load_system_font_database() -> Database {
    let mut database = Database::new();
    database.load_system_fonts();
    database
}
