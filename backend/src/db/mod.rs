use crate::config::AppConfig;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

pub type DbPool = Pool<SqliteConnectionManager>;

pub fn create_pool(config: &AppConfig) -> DbPool {
    let manager = SqliteConnectionManager::file(&config.db_path);
    let pool = Pool::new(manager).expect("Failed to create DB pool");

    let conn = pool.get().expect("Failed to get DB connection");
    run_migrations(&conn).expect("Failed to run DB migrations");

    pool
}

fn run_migrations(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS theme_settings (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_uid     INTEGER NOT NULL,
            owner_user    TEXT NOT NULL,
            theme         TEXT NOT NULL,
            font_size     INTEGER NOT NULL,
            background_opacity INTEGER NOT NULL,
            background_image   TEXT,
            background_image_type TEXT,
            saved_url_image    TEXT,
            saved_upload_image TEXT,
            tab_style     TEXT,
            cursor_style  TEXT,
            component_customization TEXT,
            is_active     INTEGER DEFAULT 1,
            updated_at    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_theme_owner_uid ON theme_settings(owner_uid);

        CREATE TABLE IF NOT EXISTS clipboard_history (
            id            TEXT PRIMARY KEY,
            owner_uid     INTEGER NOT NULL,
            kind          TEXT NOT NULL,
            text          TEXT,
            path          TEXT,
            content_type  TEXT,
            size          INTEGER,
            created_at    INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_clipboard_owner_created
            ON clipboard_history(owner_uid, created_at DESC);
    ",
    )?;
    Ok(())
}
