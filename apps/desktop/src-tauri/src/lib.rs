mod csv_metadata;

use rusqlite::{backup::Backup, params, Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{Emitter, Manager};
use walkdir::WalkDir;

const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Serialize)]
struct ScanSummary {
    root: String,
    indexed: u64,
    skipped: u64,
    errors: u64,
    missing: u64,
    reconciliation_complete: bool,
    duplicate_hash_groups: u64,
}

#[derive(Serialize, Clone)]
struct ScanProgress {
    root: String,
    visited: u64,
    indexed: u64,
    skipped: u64,
    errors: u64,
}

#[derive(Serialize)]
struct DashboardStats {
    root_count: u64,
    file_count: u64,
    duplicate_hash_groups: u64,
    drawing_metadata_count: u64,
    discrepancy_count: u64,
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
         CREATE TABLE IF NOT EXISTS scan_runs (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,
           started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           finished_at TEXT,
           status TEXT NOT NULL DEFAULT 'running'
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
           last_seen_scan_id INTEGER REFERENCES scan_runs(id),
           UNIQUE(root_id, relative_path)
         );
         CREATE INDEX IF NOT EXISTS idx_files_sha256 ON files(sha256);
         CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
         CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);
         CREATE TABLE IF NOT EXISTS file_versions (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
           sha256 TEXT NOT NULL,
           byte_size INTEGER NOT NULL,
           modified_at TEXT NOT NULL,
           captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           UNIQUE(file_id, sha256)
         );
         CREATE INDEX IF NOT EXISTS idx_file_versions_file ON file_versions(file_id, captured_at DESC);"
    ).map_err(|e| e.to_string())?;
    // Versioned migrations make future schema changes explicit and repeatable.
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version INTEGER PRIMARY KEY,
           name TEXT NOT NULL,
           applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );"
    ).map_err(|e| e.to_string())?;

    let has_last_seen: i64 = connection.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name='last_seen_scan_id'",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if has_last_seen == 0 {
        connection.execute(
            "ALTER TABLE files ADD COLUMN last_seen_scan_id INTEGER REFERENCES scan_runs(id)",
            []
        ).map_err(|e| e.to_string())?;
    }

    // Migration 1 establishes the baseline for databases created by earlier MVP builds.
    connection.execute(
        "INSERT OR IGNORE INTO schema_migrations(version, name) VALUES (1, 'baseline_scan_and_file_history')",
        []
    ).map_err(|e| e.to_string())?;

    let migration_2: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=2",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if migration_2 == 0 {
        let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
        transaction.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_files_root_status_path
               ON files(root_id, status, relative_path);
             CREATE INDEX IF NOT EXISTS idx_file_versions_hash
               ON file_versions(sha256);"
        ).map_err(|e| e.to_string())?;
        transaction.execute(
            "INSERT INTO schema_migrations(version, name) VALUES (2, 'catalog_lookup_indexes')",
            []
        ).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
    }

    apply_fts_migration(&connection)?;
    csv_metadata::ensure_schema(&connection)?;
    Ok(connection)
}

fn apply_fts_migration(connection: &Connection) -> Result<(), String> {
    let already_applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=3",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if already_applied > 0 { return Ok(()); }

    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
           filename, relative_path, content='files', content_rowid='id',
           tokenize='unicode61 remove_diacritics 2'
         );
         CREATE TRIGGER IF NOT EXISTS files_fts_ai AFTER INSERT ON files BEGIN
           INSERT INTO files_fts(rowid, filename, relative_path)
           VALUES (new.id, new.filename, new.relative_path);
         END;
         CREATE TRIGGER IF NOT EXISTS files_fts_ad AFTER DELETE ON files BEGIN
           INSERT INTO files_fts(files_fts, rowid, filename, relative_path)
           VALUES ('delete', old.id, old.filename, old.relative_path);
         END;
         CREATE TRIGGER IF NOT EXISTS files_fts_au AFTER UPDATE OF filename, relative_path ON files BEGIN
           INSERT INTO files_fts(files_fts, rowid, filename, relative_path)
           VALUES ('delete', old.id, old.filename, old.relative_path);
           INSERT INTO files_fts(rowid, filename, relative_path)
           VALUES (new.id, new.filename, new.relative_path);
         END;
         INSERT INTO files_fts(files_fts) VALUES ('rebuild');"
    ).map_err(|e| format!("Não foi possível preparar a busca textual FTS5: {e}"))?;
    transaction.execute(
        "INSERT INTO schema_migrations(version, name) VALUES (3, 'files_full_text_search')",
        []
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

fn build_fts_query(search: &str) -> Option<String> {
    let terms: Vec<String> = search
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.chars().count() >= 2)
        .map(|term| format!("\"{term}\"*"))
        .collect();
    if terms.is_empty() { None } else { Some(terms.join(" AND ")) }
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

fn scan_folder_blocking(app: tauri::AppHandle, folder: String) -> Result<ScanSummary, String> {
    let root = PathBuf::from(folder).canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() { return Err("O caminho selecionado não é uma pasta.".into()); }
    let mut connection = open_database(&app)?;
    connection.execute("INSERT OR IGNORE INTO scan_roots(path) VALUES (?1)", params![root.to_string_lossy()]).map_err(|e| e.to_string())?;
    let root_id: i64 = connection.query_row("SELECT id FROM scan_roots WHERE path=?1", params![root.to_string_lossy()], |row| row.get(0)).map_err(|e| e.to_string())?;

    connection.execute(
        "INSERT INTO scan_runs(root_id,status) VALUES (?1,'running')",
        params![root_id]
    ).map_err(|e| e.to_string())?;
    let scan_id = connection.last_insert_rowid();

    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let mut indexed = 0u64;
    let mut skipped = 0u64;
    let mut errors = 0u64;
    let mut visited = 0u64;
    let mut traversal_errors = 0u64;

    for entry_result in WalkDir::new(&root).follow_links(false) {
        let entry = match entry_result {
            Ok(value) => value,
            Err(_) => { errors += 1; traversal_errors += 1; continue; }
        };
        if !entry.file_type().is_file() { continue; }
        visited += 1;
        if visited == 1 || visited % 100 == 0 {
            let _ = app.emit("scan-progress", ScanProgress {
                root: root.to_string_lossy().to_string(),
                visited, indexed, skipped, errors
            });
        }
        let path = entry.path();
        let metadata = match path.metadata() {
            Ok(value) => value,
            Err(_) => { errors += 1; traversal_errors += 1; continue; }
        };
        let relative = match path.strip_prefix(&root) {
            Ok(value) => value.to_string_lossy().replace('\\', "/"),
            Err(_) => { errors += 1; traversal_errors += 1; continue; }
        };
        let _ = transaction.execute(
            "UPDATE files SET last_seen_scan_id=?1,status='error' WHERE root_id=?2 AND relative_path=?3",
            params![scan_id, root_id, relative]
        );
        if metadata.len() > MAX_FILE_SIZE {
            skipped += 1;
            let _ = transaction.execute(
                "UPDATE files SET last_seen_scan_id=?1,status='skipped' WHERE root_id=?2 AND relative_path=?3",
                params![scan_id, root_id, relative]
            );
            continue;
        }
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
            "INSERT INTO files(root_id,relative_path,filename,extension,byte_size,modified_at,sha256,status,last_seen_scan_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,'indexed',?8)
             ON CONFLICT(root_id,relative_path) DO UPDATE SET
             filename=excluded.filename, extension=excluded.extension,
             byte_size=excluded.byte_size, modified_at=excluded.modified_at,
             sha256=excluded.sha256, status='indexed', indexed_at=CURRENT_TIMESTAMP,
             last_seen_scan_id=excluded.last_seen_scan_id",
            params![root_id, relative, filename, extension, metadata.len() as i64, modified, hash, scan_id]
        ).is_ok() {
            let file_id: i64 = transaction.query_row(
                "SELECT id FROM files WHERE root_id=?1 AND relative_path=?2",
                params![root_id, relative], |row| row.get(0)
            ).map_err(|e| e.to_string())?;
            transaction.execute(
                "INSERT OR IGNORE INTO file_versions(file_id,sha256,byte_size,modified_at)
                 VALUES (?1,?2,?3,?4)",
                params![file_id, hash, metadata.len() as i64, modified]
            ).map_err(|e| e.to_string())?;
            indexed += 1;
            if extension == "csv" {
                match transaction.query_row(
                    "SELECT id FROM files WHERE root_id=?1 AND relative_path=?2",
                    params![root_id, relative],
                    |row| row.get::<_, i64>(0)
                ) {
                    Ok(file_id) => {
                        if csv_metadata::import_metadata_csv(&transaction, file_id, path).is_err() {
                            errors += 1;
                        }
                    }
                    Err(_) => errors += 1,
                }
            }
        } else { errors += 1; }
    }
    if traversal_errors == 0 {
        transaction.execute(
            "UPDATE files SET status='missing' WHERE root_id=?1 AND (last_seen_scan_id IS NULL OR last_seen_scan_id != ?2)",
            params![root_id, scan_id]
        ).map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    connection.execute(
        "UPDATE scan_runs SET finished_at=CURRENT_TIMESTAMP,status=?1 WHERE id=?2",
        params![if traversal_errors == 0 { "completed" } else { "partial" }, scan_id]
    ).map_err(|e| e.to_string())?;

    let missing: u64 = connection.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id=?1 AND status='missing'",
        params![root_id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let duplicate_hash_groups: u64 = connection.query_row(
        "SELECT COUNT(*) FROM (SELECT sha256 FROM files WHERE status='indexed' GROUP BY sha256 HAVING COUNT(*) > 1)",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;

    Ok(ScanSummary {
        root: root.to_string_lossy().to_string(), indexed, skipped, errors,
        missing, reconciliation_complete: traversal_errors == 0, duplicate_hash_groups
    })
}

#[tauri::command]
async fn scan_folder(app: tauri::AppHandle, folder: String) -> Result<ScanSummary, String> {
    tauri::async_runtime::spawn_blocking(move || scan_folder_blocking(app, folder))
        .await
        .map_err(|error| error.to_string())?
}


fn validate_backup_database(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err("O arquivo de backup selecionado não existe.".into());
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
    ).map_err(|e| format!("Não foi possível abrir o backup: {e}"))?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("Falha ao verificar a integridade do backup: {e}"))?;
    if integrity != "ok" {
        return Err(format!("Backup inválido: {integrity}"));
    }
    let has_files: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('scan_roots','files','file_versions')",
        [], |row| row.get(0)
    ).map_err(|e| format!("O arquivo não parece ser um banco GED: {e}"))?;
    if has_files != 3 {
        return Err("O arquivo selecionado não contém o esquema esperado do GED.".into());
    }
    let foreign_key_errors: i64 = connection.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)
    ).map_err(|e| format!("Não foi possível validar as relações do backup: {e}"))?;
    if foreign_key_errors > 0 {
        return Err(format!("Backup inválido: {foreign_key_errors} relação(ões) quebrada(s)."));
    }
    Ok(connection)
}

fn copy_database(source: &Connection, destination: &mut Connection) -> Result<(), String> {
    let backup = Backup::new(source, destination).map_err(|e| e.to_string())?;
    backup.run_to_completion(5, Duration::from_millis(100), None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_database_backup(app: tauri::AppHandle, destination_path: String) -> Result<String, String> {
    let destination_path = PathBuf::from(destination_path);
    if destination_path.extension().and_then(|value| value.to_str()).unwrap_or("").is_empty() {
        return Err("Escolha um nome de arquivo com extensão .sqlite3.".into());
    }
    let source_path = database_path(&app)?;
    if destination_path.exists() && destination_path.canonicalize().ok() == source_path.canonicalize().ok() {
        return Err("O backup não pode sobrescrever o banco de dados em uso.".into());
    }
    if let Some(parent) = destination_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let source = open_database(&app)?;
    let mut destination = Connection::open(&destination_path).map_err(|e| e.to_string())?;
    copy_database(&source, &mut destination)?;
    let integrity: String = destination.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if integrity != "ok" {
        return Err(format!("O backup foi criado, mas a verificação falhou: {integrity}"));
    }
    let foreign_key_errors: i64 = destination.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if foreign_key_errors > 0 {
        return Err(format!("O backup contém {foreign_key_errors} relação(ões) quebrada(s)."));
    }
    Ok(destination_path.to_string_lossy().to_string())
}

#[tauri::command]
fn restore_database_backup(app: tauri::AppHandle, source_path: String) -> Result<String, String> {
    let source_path = PathBuf::from(source_path).canonicalize().map_err(|e| e.to_string())?;
    let current_path = database_path(&app)?;
    if source_path == current_path.canonicalize().unwrap_or(current_path.clone()) {
        return Err("Selecione um arquivo de backup, não o banco de dados em uso.".into());
    }
    let source = validate_backup_database(&source_path)?;

    // Preserve the current database before replacing it. The safety copy is stored
    // inside the GED application data directory and can be used for manual recovery.
    let backup_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_millis();
    let safety_path = backup_dir.join(format!("ged-pre-restore-{stamp}.sqlite3"));
    let current = open_database(&app)?;
    let mut safety_copy = Connection::open(&safety_path).map_err(|e| e.to_string())?;
    copy_database(&current, &mut safety_copy)?;
    drop(safety_copy);
    drop(current);

    let mut destination = open_database(&app)?;
    let restore_result = (|| -> Result<(), String> {
        copy_database(&source, &mut destination)?;
        let integrity: String = destination.query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != "ok" {
            return Err(format!("integrity_check: {integrity}"));
        }
        let foreign_key_errors: i64 = destination.query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)
        ).map_err(|e| e.to_string())?;
        if foreign_key_errors > 0 {
            return Err(format!("{foreign_key_errors} relação(ões) quebrada(s)"));
        }
        Ok(())
    })();

    if let Err(error) = restore_result {
        let safety_source = Connection::open_with_flags(
            &safety_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        ).map_err(|rollback_error| format!(
            "A restauração falhou ({error}) e a cópia de segurança não pôde ser aberta ({rollback_error}). Arquivo preservado: {}",
            safety_path.display()
        ))?;
        copy_database(&safety_source, &mut destination).map_err(|rollback_error| format!(
            "A restauração falhou ({error}) e a recuperação automática também falhou ({rollback_error}). Cópia preservada em: {}",
            safety_path.display()
        ))?;
        return Err(format!(
            "A restauração foi cancelada e o banco anterior foi recuperado. Motivo: {error}. Cópia de segurança: {}",
            safety_path.display()
        ));
    }
    Ok(safety_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_dashboard_stats(app: tauri::AppHandle) -> Result<DashboardStats, String> {
    let connection = open_database(&app)?;
    let root_count = connection.query_row("SELECT COUNT(*) FROM scan_roots", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let file_count = connection.query_row("SELECT COUNT(*) FROM files WHERE status='indexed'", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let duplicate_hash_groups = connection.query_row(
        "SELECT COUNT(*) FROM (SELECT sha256 FROM files WHERE status='indexed' GROUP BY sha256 HAVING COUNT(*) > 1)",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let drawing_metadata_count = connection.query_row("SELECT COUNT(*) FROM drawing_metadata", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let discrepancy_count = connection.query_row("SELECT COUNT(*) FROM drawing_discrepancies", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    Ok(DashboardStats { root_count, file_count, duplicate_hash_groups, drawing_metadata_count, discrepancy_count })
}


#[derive(Serialize)]
struct FileRecord {
    id: i64,
    filename: String,
    relative_path: String,
    extension: String,
    byte_size: i64,
    modified_at: String,
    sha256: String,
    root_path: String,
    status: String,
}

#[derive(Serialize)]
struct FilePage {
    items: Vec<FileRecord>,
    total: u64,
}

#[tauri::command]
fn list_files(
    app: tauri::AppHandle,
    search: Option<String>,
    extension: Option<String>,
    sort_by: Option<String>,
    ascending: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<FilePage, String> {
    let connection = open_database(&app)?;
    let search = search.unwrap_or_default().trim().to_string();
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let fts_query = build_fts_query(&search).unwrap_or_default();
    let extension = extension.unwrap_or_default().trim().trim_start_matches('.').to_lowercase();
    let extension_filter = if extension.is_empty() { None } else { Some(extension) };

    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM files f
         WHERE (?1 = '' OR
           (?2 != '' AND f.id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?2)) OR
           (?2 = '' AND (f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')))
         AND (?4 IS NULL OR lower(f.extension) = ?4)",
        params![search, fts_query, pattern, extension_filter],
        |row| row.get(0)
    ).map_err(|e| e.to_string())?;

    let order_column = match sort_by.as_deref() {
        Some("filename") => "f.filename",
        Some("extension") => "f.extension",
        Some("size") => "f.byte_size",
        Some("modified") => "f.modified_at",
        _ => "f.relative_path",
    };
    let direction = if ascending.unwrap_or(true) { "ASC" } else { "DESC" };
    let sql = format!(
        "SELECT f.id, f.filename, f.relative_path, f.extension, f.byte_size,
                f.modified_at, f.sha256, r.path, f.status
         FROM files f JOIN scan_roots r ON r.id = f.root_id
         WHERE (?1 = '' OR
           (?2 != '' AND f.id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?2)) OR
           (?2 = '' AND (f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')))
           AND (?4 IS NULL OR lower(f.extension) = ?4)
         ORDER BY {order_column} {direction}
         LIMIT ?5 OFFSET ?6"
    );
    let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = statement.query_map(
        params![search, fts_query, pattern, extension_filter, limit.unwrap_or(100).clamp(1, 500), offset.unwrap_or(0)],
        |row| Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
            relative_path: row.get(2)?,
            extension: row.get(3)?,
            byte_size: row.get(4)?,
            modified_at: row.get(5)?,
            sha256: row.get(6)?,
            root_path: row.get(7)?,
            status: row.get(8)?,
        })
    ).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok(FilePage { items, total })
}

#[derive(Serialize)]
struct FileVersionRecord {
    id: i64,
    sha256: String,
    byte_size: i64,
    modified_at: String,
    captured_at: String,
}

#[tauri::command]
fn list_file_versions(
    app: tauri::AppHandle,
    file_id: i64,
) -> Result<Vec<FileVersionRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare(
        "SELECT id, sha256, byte_size, modified_at, captured_at
         FROM file_versions WHERE file_id=?1
         ORDER BY captured_at DESC, id DESC"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![file_id], |row| Ok(FileVersionRecord {
        id: row.get(0)?,
        sha256: row.get(1)?,
        byte_size: row.get(2)?,
        modified_at: row.get(3)?,
        captured_at: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    let mut versions = Vec::new();
    for row in rows { versions.push(row.map_err(|e| e.to_string())?); }
    Ok(versions)
}

#[derive(Serialize)]
struct DiscrepancyRecord {
    id: i64,
    filename: String,
    branch: String,
    number_in_filename: String,
    number_in_titleblock: String,
    revision: String,
    extracted_title: String,
    title_for_titleblock_number: String,
    diagnosis: String,
    artesp_number: String,
    internal_number: String,
    source_row: i64,
    source_csv: String,
}

#[derive(Serialize)]
struct DiscrepancyPage {
    items: Vec<DiscrepancyRecord>,
    total: u64,
}

#[tauri::command]
fn list_discrepancies(
    app: tauri::AppHandle,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<DiscrepancyPage, String> {
    let connection = open_database(&app)?;
    let search = search.unwrap_or_default().trim().to_string();
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM drawing_discrepancies d
         WHERE (?1 = '' OR d.filename LIKE ?2 ESCAPE '\\'
           OR d.branch LIKE ?2 ESCAPE '\\' OR d.diagnosis LIKE ?2 ESCAPE '\\'
           OR d.extracted_title LIKE ?2 ESCAPE '\\'
           OR d.title_for_titleblock_number LIKE ?2 ESCAPE '\\')",
        params![search, pattern], |row| row.get(0)
    ).map_err(|e| e.to_string())?;

    let mut statement = connection.prepare(
        "SELECT d.id, d.filename, d.branch, d.number_in_filename,
                d.number_in_titleblock, d.revision, d.extracted_title,
                d.title_for_titleblock_number, d.diagnosis, d.artesp_number,
                d.internal_number, d.source_row, f.filename
         FROM drawing_discrepancies d
         JOIN files f ON f.id = d.source_file_id
         WHERE (?1 = '' OR d.filename LIKE ?2 ESCAPE '\\'
           OR d.branch LIKE ?2 ESCAPE '\\' OR d.diagnosis LIKE ?2 ESCAPE '\\'
           OR d.extracted_title LIKE ?2 ESCAPE '\\'
           OR d.title_for_titleblock_number LIKE ?2 ESCAPE '\\')
         ORDER BY d.branch ASC, d.filename ASC
         LIMIT ?3 OFFSET ?4"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(
        params![search, pattern, limit.unwrap_or(100).clamp(1, 500), offset.unwrap_or(0)],
        |row| Ok(DiscrepancyRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
            branch: row.get(2)?,
            number_in_filename: row.get(3)?,
            number_in_titleblock: row.get(4)?,
            revision: row.get(5)?,
            extracted_title: row.get(6)?,
            title_for_titleblock_number: row.get(7)?,
            diagnosis: row.get(8)?,
            artesp_number: row.get(9)?,
            internal_number: row.get(10)?,
            source_row: row.get(11)?,
            source_csv: row.get(12)?,
        })
    ).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok(DiscrepancyPage { items, total })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, get_dashboard_stats, create_database_backup, restore_database_backup, list_files, list_file_versions, list_discrepancies])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o GED Desktop");
}


#[cfg(test)]
mod backup_tests {
    use super::*;

    #[test]
    fn fts_migration_indexes_existing_and_future_files() {
        let connection = Connection::open_in_memory().expect("open database");
        connection.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE files(id INTEGER PRIMARY KEY, filename TEXT NOT NULL, relative_path TEXT NOT NULL);
             INSERT INTO files(id, filename, relative_path) VALUES (1, 'Bridge-Plan.dwg', 'AB20A/Bridge-Plan.dwg');"
        ).expect("create legacy schema");

        apply_fts_migration(&connection).expect("apply FTS migration");
        let query = build_fts_query("Bridge-Plan").expect("build query");
        let existing: i64 = connection.query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH ?1",
            params![query], |row| row.get(0)
        ).expect("search existing row");
        assert_eq!(existing, 1);

        connection.execute(
            "INSERT INTO files(id, filename, relative_path) VALUES (2, 'Road-Section.pdf', 'AB20A/Road-Section.pdf')",
            []
        ).expect("insert new row");
        let future: i64 = connection.query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH 'Road*'",
            [], |row| row.get(0)
        ).expect("search inserted row");
        assert_eq!(future, 1);
    }

    #[test]
    fn fts_migration_indexes_existing_and_future_files() {
        let connection = Connection::open_in_memory().expect("open database");
        connection.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE files(id INTEGER PRIMARY KEY, filename TEXT NOT NULL, relative_path TEXT NOT NULL);
             INSERT INTO files(id, filename, relative_path) VALUES (1, 'Bridge-Plan.dwg', 'AB20A/Bridge-Plan.dwg');"
        ).expect("create legacy schema");

        apply_fts_migration(&connection).expect("apply FTS migration");
        let query = build_fts_query("Bridge-Plan").expect("build query");
        let existing: i64 = connection.query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH ?1",
            params![query], |row| row.get(0)
        ).expect("search existing row");
        assert_eq!(existing, 1);

        connection.execute(
            "INSERT INTO files(id, filename, relative_path) VALUES (2, 'Road-Section.pdf', 'AB20A/Road-Section.pdf')",
            []
        ).expect("insert new row");
        let future: i64 = connection.query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH 'Road*'",
            [], |row| row.get(0)
        ).expect("search inserted row");
        assert_eq!(future, 1);
    }

    #[test]
    fn sqlite_online_backup_preserves_schema_and_rows() {
        let source = Connection::open_in_memory().expect("open source");
        source.execute_batch(
            "CREATE TABLE sample(id INTEGER PRIMARY KEY, label TEXT NOT NULL);
             INSERT INTO sample(label) VALUES ('GED backup test');"
        ).expect("seed source");
        let mut destination = Connection::open_in_memory().expect("open destination");

        copy_database(&source, &mut destination).expect("copy database");

        let label: String = destination.query_row(
            "SELECT label FROM sample WHERE id=1", [], |row| row.get(0)
        ).expect("read restored row");
        assert_eq!(label, "GED backup test");
        let integrity: String = destination.query_row(
            "PRAGMA integrity_check", [], |row| row.get(0)
        ).expect("check integrity");
        assert_eq!(integrity, "ok");
    }
}
