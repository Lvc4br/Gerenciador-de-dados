mod csv_metadata;
mod projects;
mod suggestions;

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
    apply_document_text_migration(&connection)?;
    apply_pdf_page_count_migration(&connection)?;
    apply_pdf_metadata_migration(&connection)?;
    csv_metadata::ensure_schema(&connection)?;
    projects::ensure_schema(&connection)?;
    projects::ensure_relations_schema(&connection)?;
    suggestions::ensure_schema(&connection)?;
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

fn apply_document_text_migration(connection: &Connection) -> Result<(), String> {
    let already_applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=6",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if already_applied > 0 { return Ok(()); }

    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS extracted_document_text (
           file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
           sha256 TEXT NOT NULL,
           content TEXT NOT NULL,
           character_count INTEGER NOT NULL,
           parser_version TEXT NOT NULL,
           extracted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS document_text_fts USING fts5(
           content, content='extracted_document_text', content_rowid='file_id',
           tokenize='unicode61 remove_diacritics 2'
         );
         CREATE TRIGGER IF NOT EXISTS document_text_ai AFTER INSERT ON extracted_document_text BEGIN
           INSERT INTO document_text_fts(rowid, content) VALUES (new.file_id, new.content);
         END;
         CREATE TRIGGER IF NOT EXISTS document_text_ad AFTER DELETE ON extracted_document_text BEGIN
           INSERT INTO document_text_fts(document_text_fts, rowid, content)
           VALUES ('delete', old.file_id, old.content);
         END;
         CREATE TRIGGER IF NOT EXISTS document_text_au AFTER UPDATE ON extracted_document_text BEGIN
           INSERT INTO document_text_fts(document_text_fts, rowid, content)
           VALUES ('delete', old.file_id, old.content);
           INSERT INTO document_text_fts(rowid, content) VALUES (new.file_id, new.content);
         END;
         INSERT INTO document_text_fts(document_text_fts) VALUES ('rebuild');"
    ).map_err(|e| format!("Não foi possível preparar o índice de conteúdo PDF: {e}"))?;
    transaction.execute(
        "INSERT INTO schema_migrations(version, name) VALUES (6, 'pdf_document_text_search')",
        []
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

fn apply_pdf_page_count_migration(connection: &Connection) -> Result<(), String> {
    let already_applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=7",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if already_applied > 0 { return Ok(()); }
    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "ALTER TABLE extracted_document_text ADD COLUMN page_count INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE extracted_document_text ADD COLUMN pages_with_text INTEGER NOT NULL DEFAULT 0;"
    ).map_err(|e| format!("Não foi possível ampliar os metadados de extração PDF: {e}"))?;
    transaction.execute(
        "INSERT INTO schema_migrations(version, name) VALUES (7, 'pdf_extracted_page_counts')",
        []
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

fn apply_pdf_metadata_migration(connection: &Connection) -> Result<(), String> {
    let already_applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=8",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if already_applied > 0 { return Ok(()); }
    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "ALTER TABLE extracted_document_text ADD COLUMN drawing_number TEXT;
         ALTER TABLE extracted_document_text ADD COLUMN revision TEXT;
         ALTER TABLE extracted_document_text ADD COLUMN candidate_title TEXT;
         ALTER TABLE extracted_document_text ADD COLUMN metadata_confidence REAL NOT NULL DEFAULT 0;"
    ).map_err(|e| format!("Não foi possível preparar os metadados candidatos do PDF: {e}"))?;
    transaction.execute(
        "INSERT INTO schema_migrations(version, name) VALUES (8, 'pdf_metadata_candidates')",
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
            suggestions::analyze_and_store(&transaction, file_id, &filename)?;
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
    content_snippet: Option<String>,
    pdf_text_indexed: bool,
    pdf_text_characters: Option<i64>,
    pdf_page_count: Option<i64>,
    pdf_extracted_at: Option<String>,
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
    project_id: Option<i64>,
    discipline_id: Option<i64>,
    client: Option<String>,
    sort_by: Option<String>,
    ascending: Option<bool>,
    search_in_content: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<FilePage, String> {
    let connection = open_database(&app)?;
    let search = search.unwrap_or_default().trim().to_string();
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let fts_query = build_fts_query(&search).unwrap_or_default();
    let extension = extension.unwrap_or_default().trim().trim_start_matches('.').to_lowercase();
    let extension_filter = if extension.is_empty() { None } else { Some(extension) };
    let client = client.unwrap_or_default().trim().to_string();
    let client_pattern = format!("%{}%", client.replace('%', "\\%").replace('_', "\\_"));

    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM files f
         WHERE (?1 = '' OR
           (?2 != '' AND f.id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?2)) OR
           (?9 = 1 AND ?2 != '' AND f.id IN (
             SELECT document_text_fts.rowid FROM document_text_fts
             JOIN extracted_document_text x ON x.file_id=document_text_fts.rowid
             JOIN files current_file ON current_file.id=x.file_id
             WHERE document_text_fts MATCH ?2 AND x.sha256=current_file.sha256
           )) OR
           (?2 = '' AND (f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')))
         AND (?4 IS NULL OR lower(f.extension) = ?4)
         AND (?5 IS NULL OR EXISTS (
           SELECT 1 FROM project_roots pr WHERE pr.project_id=?5 AND pr.root_id=f.root_id
           UNION
           SELECT 1 FROM project_file_links pfl WHERE pfl.project_id=?5 AND pfl.file_id=f.id
         ))
         AND (?6 IS NULL OR EXISTS (
           SELECT 1 FROM project_file_links pfl
           WHERE pfl.project_id=COALESCE(?5,pfl.project_id) AND pfl.file_id=f.id AND pfl.discipline_id=?6
         ))
         AND (?7='' OR EXISTS (
           SELECT 1 FROM projects p
           LEFT JOIN project_roots pr ON pr.project_id=p.id AND pr.root_id=f.root_id
           LEFT JOIN project_file_links pfl ON pfl.project_id=p.id AND pfl.file_id=f.id
           WHERE (pr.project_id IS NOT NULL OR pfl.project_id IS NOT NULL)
             AND p.client LIKE ?8 ESCAPE '\\'
         ))",
        params![search, fts_query, pattern, extension_filter, project_id, discipline_id, client, client_pattern, search_in_content.unwrap_or(false)],
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
                f.modified_at, f.sha256, r.path, f.status,
                CASE WHEN ?9=1 AND ?2!='' THEN (
                  SELECT snippet(document_text_fts,0,'[',']',' … ',18)
                  FROM document_text_fts
                  WHERE document_text_fts MATCH ?2 AND rowid=f.id
                ) ELSE NULL END,
                EXISTS(SELECT 1 FROM extracted_document_text x WHERE x.file_id=f.id AND x.sha256=f.sha256),
                (SELECT x.character_count FROM extracted_document_text x WHERE x.file_id=f.id AND x.sha256=f.sha256),
                (SELECT x.page_count FROM extracted_document_text x WHERE x.file_id=f.id AND x.sha256=f.sha256),
                (SELECT x.extracted_at FROM extracted_document_text x WHERE x.file_id=f.id AND x.sha256=f.sha256)
         FROM files f JOIN scan_roots r ON r.id = f.root_id
         WHERE (?1 = '' OR
           (?2 != '' AND f.id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?2)) OR
           (?9 = 1 AND ?2 != '' AND f.id IN (
             SELECT document_text_fts.rowid FROM document_text_fts
             JOIN extracted_document_text x ON x.file_id=document_text_fts.rowid
             JOIN files current_file ON current_file.id=x.file_id
             WHERE document_text_fts MATCH ?2 AND x.sha256=current_file.sha256
           )) OR
           (?2 = '' AND (f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')))
           AND (?4 IS NULL OR lower(f.extension) = ?4)
           AND (?5 IS NULL OR EXISTS (
             SELECT 1 FROM project_roots pr WHERE pr.project_id=?5 AND pr.root_id=f.root_id
             UNION
             SELECT 1 FROM project_file_links pfl WHERE pfl.project_id=?5 AND pfl.file_id=f.id
           ))
           AND (?6 IS NULL OR EXISTS (
             SELECT 1 FROM project_file_links pfl
             WHERE pfl.project_id=COALESCE(?5,pfl.project_id) AND pfl.file_id=f.id AND pfl.discipline_id=?6
           ))
           AND (?7='' OR EXISTS (
             SELECT 1 FROM projects p
             LEFT JOIN project_roots pr ON pr.project_id=p.id AND pr.root_id=f.root_id
             LEFT JOIN project_file_links pfl ON pfl.project_id=p.id AND pfl.file_id=f.id
             WHERE (pr.project_id IS NOT NULL OR pfl.project_id IS NOT NULL)
               AND p.client LIKE ?8 ESCAPE '\\'
           ))
         ORDER BY {order_column} {direction}
         LIMIT ?10 OFFSET ?11"
    );
    let mut statement = connection.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = statement.query_map(
        params![search, fts_query, pattern, extension_filter, project_id, discipline_id, client, client_pattern, search_in_content.unwrap_or(false), limit.unwrap_or(100).clamp(1, 500), offset.unwrap_or(0)],
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
            content_snippet: row.get(9)?,
            pdf_text_indexed: row.get(10)?,
            pdf_text_characters: row.get(11)?,
            pdf_page_count: row.get(12)?,
            pdf_extracted_at: row.get(13)?,
        })
    ).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok(FilePage { items, total })
}

fn extract_pdf_metadata_candidates(text: &str) -> Result<(Option<String>, Option<String>, Option<String>, f64), String> {
    let drawing_pattern = regex::Regex::new(r"(?i)\b([A-Z]{2,5})[-_ ]?(\d{2,5})\b")
        .map_err(|e| e.to_string())?;
    let revision_pattern = regex::Regex::new(r"(?i)\bREV(?:IS[ÃA]O)?[ .:_-]*([A-Z0-9]{1,4})\b")
        .map_err(|e| e.to_string())?;
    let title_pattern = regex::Regex::new(r"(?im)^\s*(?:T[IÍ]TULO|TITULO|DESCRI[CÇ][AÃ]O|NOME\s+DO\s+DESENHO)\s*[:\-]\s*(.{5,180})\s*$")
        .map_err(|e| e.to_string())?;

    let drawing_number = drawing_pattern.captures(text).and_then(|captures| {
        let prefix = captures.get(1)?.as_str().to_ascii_uppercase();
        if ["REV", "ISO", "ABNT", "NBR", "BR"].contains(&prefix.as_str()) { return None; }
        let number = captures.get(2)?.as_str();
        Some(format!("{prefix}-{number}"))
    });
    let revision = revision_pattern.captures(text)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_ascii_uppercase()));
    let candidate_title = title_pattern.captures(text)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().trim().to_string()))
        .filter(|value| !value.is_empty());
    let confidence = match (drawing_number.is_some(), revision.is_some(), candidate_title.is_some()) {
        (true, true, true) => 0.86,
        (true, true, false) => 0.82,
        (true, false, true) => 0.76,
        (true, false, false) => 0.68,
        (false, true, true) => 0.48,
        (false, true, false) => 0.35,
        (false, false, true) => 0.40,
        (false, false, false) => 0.0,
    };
    Ok((drawing_number, revision, candidate_title, confidence))
}

const MAX_PDF_BYTES: u64 = 200 * 1024 * 1024;
const MAX_PDF_TEXT_CHARACTERS: usize = 10_000_000;

fn extract_pdf_text_blocking(app: tauri::AppHandle, file_id: i64) -> Result<serde_json::Value, String> {
    let mut connection = open_database(&app)?;
    let (root_path, relative_path, extension, expected_hash): (String, String, String, String) =
        connection.query_row(
            "SELECT r.path, f.relative_path, f.extension, f.sha256
             FROM files f JOIN scan_roots r ON r.id=f.root_id WHERE f.id=?1 AND f.status='indexed'",
            params![file_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        ).map_err(|error| format!("Documento não encontrado ou não está indexado: {error}"))?;
    if extension.to_lowercase() != "pdf" {
        return Err("A extração de conteúdo está disponível apenas para arquivos PDF.".into());
    }
    let file_path = PathBuf::from(root_path).join(relative_path);
    let metadata = std::fs::metadata(&file_path).map_err(|e| format!("Não foi possível acessar o PDF: {e}"))?;
    if metadata.len() > MAX_PDF_BYTES {
        return Err("PDF excede o limite de 200 MB para extração.".into());
    }
    let current_hash = hash_file(&file_path)?;
    if current_hash != expected_hash {
        return Err("O PDF foi alterado desde a última varredura. Execute uma nova indexação antes de extrair.".into());
    }
    let path_string = file_path.to_string_lossy().to_string();
    let pages = pdf_extract::extract_text_by_pages(&path_string)
        .map_err(|e| format!("Não foi possível extrair o texto do PDF: {e}"))?;
    if pages.len() > 5000 {
        return Err("PDF excede o limite de 5.000 páginas para extração.".into());
    }
    let page_count = pages.len();
    let mut pages_with_text = 0usize;
    let mut character_count = 0usize;
    let mut content = String::new();
    for (index, page_text) in pages.iter().enumerate() {
        let page_text = page_text.replace("\r\n", "\n").replace('\r', "\n");
        if !page_text.trim().is_empty() {
            pages_with_text += 1;
            character_count += page_text.chars().count();
            if character_count > MAX_PDF_TEXT_CHARACTERS {
                return Err(format!("Texto extraído excede o limite de {MAX_PDF_TEXT_CHARACTERS} caracteres."));
            }
            content.push_str(&format!("\n\n--- Página {} ---\n{}", index + 1, page_text));
        }
    }
    let (drawing_number, revision, candidate_title, metadata_confidence) =
        extract_pdf_metadata_candidates(&content)?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    transaction.execute(
        "INSERT INTO extracted_document_text(
           file_id,sha256,content,character_count,parser_version,page_count,pages_with_text,
           drawing_number,revision,candidate_title,metadata_confidence
         ) VALUES (?1,?2,?3,?4,'pdf-extract-0.12.1',?5,?6,?7,?8,?9,?10)
         ON CONFLICT(file_id) DO UPDATE SET sha256=excluded.sha256,
           content=excluded.content, character_count=excluded.character_count,
           parser_version=excluded.parser_version, page_count=excluded.page_count,
           pages_with_text=excluded.pages_with_text, drawing_number=excluded.drawing_number,
           revision=excluded.revision, candidate_title=excluded.candidate_title,
           metadata_confidence=excluded.metadata_confidence, extracted_at=CURRENT_TIMESTAMP",
        params![file_id, expected_hash, content, character_count as i64, page_count as i64,
                pages_with_text as i64, drawing_number, revision, candidate_title, metadata_confidence]
    ).map_err(|e| format!("Não foi possível salvar o texto extraído: {e}"))?;

    if drawing_number.is_some() || revision.is_some() {
        let has_filename_suggestion: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM filename_suggestions WHERE file_id=?1 AND parser_version='1')",
            params![file_id], |row| row.get(0)
        ).map_err(|e| e.to_string())?;
        if !has_filename_suggestion {
            let discipline = drawing_number.as_deref().and_then(|value| value.split('-').next());
            transaction.execute(
                "INSERT INTO filename_suggestions(
                   file_id,discipline_code,drawing_number,revision,confidence,source,parser_version
                 ) VALUES (?1,?2,?3,?4,?5,'pdf_content','pdf-content-1')
                 ON CONFLICT(file_id,parser_version) DO UPDATE SET
                   discipline_code=excluded.discipline_code,drawing_number=excluded.drawing_number,
                   revision=excluded.revision,confidence=excluded.confidence,
                   status=CASE
                     WHEN filename_suggestions.discipline_code IS excluded.discipline_code
                      AND filename_suggestions.drawing_number IS excluded.drawing_number
                      AND filename_suggestions.revision IS excluded.revision
                      AND filename_suggestions.confidence=excluded.confidence
                     THEN filename_suggestions.status ELSE 'pending' END,
                   reviewed_at=CASE
                     WHEN filename_suggestions.discipline_code IS excluded.discipline_code
                      AND filename_suggestions.drawing_number IS excluded.drawing_number
                      AND filename_suggestions.revision IS excluded.revision
                      AND filename_suggestions.confidence=excluded.confidence
                     THEN filename_suggestions.reviewed_at ELSE NULL END",
                params![file_id, discipline, drawing_number, revision, metadata_confidence]
            ).map_err(|e| format!("Não foi possível registrar a sugestão extraída do PDF: {e}"))?;
        }
    }
    transaction.commit().map_err(|e| format!("Não foi possível confirmar os dados extraídos: {e}"))?;
    Ok(serde_json::json!({
        "file_id": file_id,
        "character_count": character_count,
        "page_count": page_count,
        "pages_with_text": pages_with_text,
        "needs_ocr": pages_with_text == 0,
        "drawing_number": drawing_number,
        "revision": revision,
        "candidate_title": candidate_title,
        "metadata_confidence": metadata_confidence,
        "parser_version": "pdf-extract-0.12.1"
    }))
}

#[tauri::command]
async fn extract_pdf_text(app: tauri::AppHandle, file_id: i64) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || extract_pdf_text_blocking(app, file_id))
        .await
        .map_err(|error| error.to_string())?
}

fn extract_pending_pdf_text_blocking(app: tauri::AppHandle, limit: u32) -> Result<serde_json::Value, String> {
    let connection = open_database(&app)?;
    let mut statement = connection.prepare(
        "SELECT f.id FROM files f
         LEFT JOIN extracted_document_text x ON x.file_id=f.id
         WHERE f.status='indexed' AND lower(f.extension)='pdf'
           AND (x.file_id IS NULL OR x.sha256 != f.sha256)
         ORDER BY f.id LIMIT ?1"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![limit.clamp(1, 200)], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    for row in rows { ids.push(row.map_err(|e| e.to_string())?); }
    drop(statement);
    drop(connection);

    let mut processed = 0u64;
    let mut needs_ocr = 0u64;
    let mut errors: Vec<serde_json::Value> = Vec::new();
    for file_id in ids {
        match extract_pdf_text_blocking(app.clone(), file_id) {
            Ok(result) => {
                processed += 1;
                if result.get("needs_ocr").and_then(|value| value.as_bool()).unwrap_or(false) {
                    needs_ocr += 1;
                }
            }
            Err(error) => errors.push(serde_json::json!({ "file_id": file_id, "error": error })),
        }
    }
    Ok(serde_json::json!({
        "processed": processed,
        "needs_ocr": needs_ocr,
        "errors": errors.len(),
        "error_details": errors
    }))
}

#[tauri::command]
async fn extract_pending_pdf_text(app: tauri::AppHandle, limit: Option<u32>) -> Result<serde_json::Value, String> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    tauri::async_runtime::spawn_blocking(move || extract_pending_pdf_text_blocking(app, limit))
        .await
        .map_err(|error| error.to_string())?
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


#[tauri::command]
fn list_filename_suggestions(
    app: tauri::AppHandle, status: Option<String>, search: Option<String>,
    limit: Option<u32>, offset: Option<u32>,
) -> Result<serde_json::Value, String> {
    let connection = open_database(&app)?;
    let (items, total) = suggestions::list(
        &connection, status.as_deref().unwrap_or("pending"), search.as_deref().unwrap_or(""),
        limit.unwrap_or(100), offset.unwrap_or(0)
    )?;
    Ok(serde_json::json!({ "items": items, "total": total }))
}

#[tauri::command]
fn review_filename_suggestion(
    app: tauri::AppHandle, id: i64, decision: String,
) -> Result<(), String> {
    let connection = open_database(&app)?;
    suggestions::review(&connection, id, &decision)
}

#[tauri::command]
fn apply_accepted_filename_suggestion(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    suggestions::apply_accepted(&connection, id)
}

#[tauri::command]
fn list_projects(
    app: tauri::AppHandle,
    search: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<serde_json::Value, String> {
    let connection = open_database(&app)?;
    let (items, total) = projects::list(
        &connection, search.as_deref().unwrap_or(""), status.as_deref().unwrap_or(""),
        limit.unwrap_or(100), offset.unwrap_or(0)
    )?;
    serde_json::to_value(serde_json::json!({ "items": items, "total": total })).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_project(
    app: tauri::AppHandle, code: String, name: String,
    client: Option<String>, description: Option<String>,
) -> Result<i64, String> {
    let connection = open_database(&app)?;
    projects::create(&connection, &code, &name, client.as_deref().unwrap_or(""), description.as_deref().unwrap_or(""))
}

#[tauri::command]
fn update_project(
    app: tauri::AppHandle, id: i64, code: String, name: String,
    client: Option<String>, description: Option<String>,
) -> Result<(), String> {
    let connection = open_database(&app)?;
    projects::update(&connection, id, &code, &name, client.as_deref().unwrap_or(""), description.as_deref().unwrap_or(""))
}

#[tauri::command]
fn archive_project(app: tauri::AppHandle, id: i64, archived: bool) -> Result<(), String> {
    let connection = open_database(&app)?;
    projects::set_archived(&connection, id, archived)
}

#[tauri::command]
fn list_project_roots(app: tauri::AppHandle, project_id: i64) -> Result<Vec<projects::ProjectRootRecord>, String> {
    let connection = open_database(&app)?;
    projects::list_roots(&connection, project_id)
}

#[tauri::command]
fn set_project_root_link(app: tauri::AppHandle, project_id: i64, root_id: i64, linked: bool) -> Result<(), String> {
    let connection = open_database(&app)?;
    projects::link_root(&connection, project_id, root_id, linked)
}


#[tauri::command]
fn list_project_disciplines(app: tauri::AppHandle, project_id: i64) -> Result<Vec<projects::ProjectDisciplineRecord>, String> {
    let connection = open_database(&app)?;
    projects::list_disciplines(&connection, project_id)
}

#[tauri::command]
fn save_project_discipline(
    app: tauri::AppHandle, project_id: i64, id: Option<i64>,
    code: String, name: String, sort_order: Option<i64>,
) -> Result<i64, String> {
    let connection = open_database(&app)?;
    projects::save_discipline(&connection, project_id, id, &code, &name, sort_order.unwrap_or(0))
}

#[tauri::command]
fn delete_project_discipline(app: tauri::AppHandle, project_id: i64, discipline_id: i64) -> Result<(), String> {
    let connection = open_database(&app)?;
    projects::delete_discipline(&connection, project_id, discipline_id)
}

#[tauri::command]
fn list_project_files(
    app: tauri::AppHandle, project_id: i64, search: Option<String>,
    discipline_id: Option<i64>, limit: Option<u32>, offset: Option<u32>,
) -> Result<serde_json::Value, String> {
    let connection = open_database(&app)?;
    let (items,total) = projects::list_project_files(
        &connection, project_id, search.as_deref().unwrap_or(""), discipline_id,
        limit.unwrap_or(100), offset.unwrap_or(0)
    )?;
    Ok(serde_json::json!({ "items": items, "total": total }))
}

#[tauri::command]
fn set_project_file_link(
    app: tauri::AppHandle, project_id: i64, file_id: i64,
    linked: bool, discipline_id: Option<i64>,
) -> Result<(), String> {
    let connection = open_database(&app)?;
    projects::set_project_file_link(&connection, project_id, file_id, linked, discipline_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, get_dashboard_stats, create_database_backup, restore_database_backup, list_files, extract_pdf_text, extract_pending_pdf_text, list_file_versions, list_discrepancies, list_filename_suggestions, review_filename_suggestion, apply_accepted_filename_suggestion, list_projects, create_project, update_project, archive_project, list_project_roots, set_project_root_link, list_project_disciplines, save_project_discipline, delete_project_discipline, list_project_files, set_project_file_link])
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
    fn pdf_extract_reads_text_by_page_from_a_real_pdf_file() {
        let path = std::env::temp_dir().join(format!("ged-pdf-fixture-{}.pdf", std::process::id()));
        let stream = "BT /F1 12 Tf 50 700 Td (EST-001 REV02) Tj ET";
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = vec![0usize];
        let objects = [
            "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n".to_string(),
            "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n".to_string(),
            "3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >> endobj\n".to_string(),
            "4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n".to_string(),
            format!("5 0 obj << /Length {} >> stream\n{}\nendstream endobj\n", stream.len(), stream),
        ];
        for object in objects {
            offsets.push(pdf.len());
            pdf.push_str(&object);
        }
        let xref_offset = pdf.len();
        pdf.push_str(&format!("xref\n0 {}\n", offsets.len()));
        pdf.push_str("0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.push_str(&format!("{:010} 00000 n \n", offset));
        }
        pdf.push_str(&format!(
            "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len(), xref_offset
        ));
        std::fs::write(&path, pdf).expect("write test PDF");

        let pages = pdf_extract::extract_text_by_pages(&path).expect("extract PDF pages");
        let _ = std::fs::remove_file(&path);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("EST-001"));
        assert!(pages[0].contains("REV02"));
    }

    #[test]
    fn pdf_metadata_candidates_detect_drawing_revision_and_title() {
        let text = "Número do desenho: EST-001\nRevisão: REV02\nTítulo: Planta de formas do tabuleiro";
        let (drawing, revision, title, confidence) =
            extract_pdf_metadata_candidates(text).expect("parse PDF metadata");
        assert_eq!(drawing.as_deref(), Some("EST-001"));
        assert_eq!(revision.as_deref(), Some("02"));
        assert_eq!(title.as_deref(), Some("Planta de formas do tabuleiro"));
        assert_eq!(confidence, 0.86);
    }

    #[test]
    fn pdf_metadata_candidates_do_not_treat_iso_standard_as_drawing_number() {
        let (drawing, revision, title, confidence) =
            extract_pdf_metadata_candidates("Norma ISO-9001 e texto genérico").expect("parse PDF metadata");
        assert_eq!(drawing, None);
        assert_eq!(revision, None);
        assert_eq!(title, None);
        assert_eq!(confidence, 0.0);
    }

    #[test]
    fn document_text_migration_indexes_and_updates_pdf_content() {
        let connection = Connection::open_in_memory().expect("open database");
        connection.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE files(id INTEGER PRIMARY KEY, filename TEXT NOT NULL, relative_path TEXT NOT NULL);"
        ).expect("create legacy schema");
        apply_document_text_migration(&connection).expect("apply document text migration");
        apply_pdf_page_count_migration(&connection).expect("apply PDF page count migration");
        apply_pdf_metadata_migration(&connection).expect("apply PDF metadata migration");
        connection.execute(
            "INSERT INTO files(id,filename,relative_path) VALUES (1,'memorial.pdf','memorial.pdf')", []
        ).expect("insert file");
        connection.execute(
            "INSERT INTO extracted_document_text(file_id,sha256,content,character_count,parser_version,page_count,pages_with_text)
             VALUES (1,'abc','Estrutura de concreto EST-001',30,'test',2,2)", []
        ).expect("insert extracted text");
        let original_query = "\\"EST-001\\"";
        let found: i64 = connection.query_row(
            "SELECT COUNT(*) FROM document_text_fts WHERE document_text_fts MATCH ?1",
            params![original_query], |row| row.get(0)
        ).expect("search extracted text");
        assert_eq!(found, 1);

        connection.execute(
            "UPDATE extracted_document_text SET content='Memorial de drenagem DRE-014' WHERE file_id=1", []
        ).expect("update extracted text");
        let stale: i64 = connection.query_row(
            "SELECT COUNT(*) FROM document_text_fts WHERE document_text_fts MATCH ?1",
            params![original_query], |row| row.get(0)
        ).expect("search stale content");
        let updated_query = "\\"DRE-014\\"";
        let current: i64 = connection.query_row(
            "SELECT COUNT(*) FROM document_text_fts WHERE document_text_fts MATCH ?1",
            params![updated_query], |row| row.get(0)
        ).expect("search current content");
        assert_eq!(stale, 0);
        assert_eq!(current, 1);
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
