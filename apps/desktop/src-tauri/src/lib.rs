use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::{Path, PathBuf}};
use tauri::Manager;
use walkdir::WalkDir;

const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Serialize)]
struct ScanSummary {
    root: String,
    indexed: u64,
    skipped: u64,
    errors: u64,
    duplicate_hash_groups: u64,
}

#[derive(Serialize)]
struct DashboardStats {
    root_count: u64,
    file_count: u64,
    duplicate_hash_groups: u64,
}

fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut directory = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    directory.push("ged.sqlite3");
    Ok(directory)
}

fn open_database(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    let connection = Connection::open(path).map_err(|e| e.to_string())?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS scan_roots (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           path TEXT NOT NULL UNIQUE,
           added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS files (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,
           relative_path TEXT NOT NULL,
           filename TEXT NOT NULL,
           extension TEXT NOT NULL,
           byte_size INTEGER NOT NULL,
           modified_at TEXT NOT NULL,
           sha256 TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'indexed',
           indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           UNIQUE(root_id, relative_path)
         );
         CREATE INDEX IF NOT EXISTS idx_files_sha256 ON files(sha256);
         CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
         CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);"
    ).map_err(|e| e.to_string())?;
    Ok(connection)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[tauri::command]
fn scan_folder(app: tauri::AppHandle, folder: String) -> Result<ScanSummary, String> {
    let root = PathBuf::from(folder).canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() { return Err("O caminho selecionado não é uma pasta.".into()); }
    let mut connection = open_database(&app)?;
    connection.execute("INSERT OR IGNORE INTO scan_roots(path) VALUES (?1)", params![root.to_string_lossy()]).map_err(|e| e.to_string())?;
    let root_id: i64 = connection.query_row("SELECT id FROM scan_roots WHERE path=?1", params![root.to_string_lossy()], |row| row.get(0)).map_err(|e| e.to_string())?;

    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let mut indexed = 0u64;
    let mut skipped = 0u64;
    let mut errors = 0u64;

    for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        let metadata = match path.metadata() {
            Ok(value) => value,
            Err(_) => { errors += 1; continue; }
        };
        if metadata.len() > MAX_FILE_SIZE { skipped += 1; continue; }
        let relative = match path.strip_prefix(&root) {
            Ok(value) => value.to_string_lossy().replace('\\', "/"),
            Err(_) => { errors += 1; continue; }
        };
        let hash = match hash_file(path) {
            Ok(value) => value,
            Err(_) => { errors += 1; continue; }
        };
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let extension = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
        let modified = metadata.modified().ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_default();

        if transaction.execute(
            "INSERT INTO files(root_id,relative_path,filename,extension,byte_size,modified_at,sha256,status)
             VALUES (?1,?2,?3,?4,?5,?6,?7,'indexed')
             ON CONFLICT(root_id,relative_path) DO UPDATE SET
             filename=excluded.filename, extension=excluded.extension,
             byte_size=excluded.byte_size, modified_at=excluded.modified_at,
             sha256=excluded.sha256, status='indexed', indexed_at=CURRENT_TIMESTAMP",
            params![root_id, relative, filename, extension, metadata.len() as i64, modified, hash]
        ).is_ok() { indexed += 1; } else { errors += 1; }
    }
    transaction.commit().map_err(|e| e.to_string())?;

    let duplicate_hash_groups: u64 = connection.query_row(
        "SELECT COUNT(*) FROM (SELECT sha256 FROM files GROUP BY sha256 HAVING COUNT(*) > 1)",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;

    Ok(ScanSummary { root: root.to_string_lossy().to_string(), indexed, skipped, errors, duplicate_hash_groups })
}

#[tauri::command]
fn get_dashboard_stats(app: tauri::AppHandle) -> Result<DashboardStats, String> {
    let connection = open_database(&app)?;
    let root_count = connection.query_row("SELECT COUNT(*) FROM scan_roots", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let file_count = connection.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let duplicate_hash_groups = connection.query_row(
        "SELECT COUNT(*) FROM (SELECT sha256 FROM files GROUP BY sha256 HAVING COUNT(*) > 1)",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    Ok(DashboardStats { root_count, file_count, duplicate_hash_groups })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, get_dashboard_stats])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o GED Desktop");
}
