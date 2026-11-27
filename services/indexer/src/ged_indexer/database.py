from __future__ import annotations

import sqlite3
from pathlib import Path

SCHEMA = """
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
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
CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);
CREATE TABLE IF NOT EXISTS findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    rule_code TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    details TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(file_id, rule_code, details)
);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    filename, relative_path, content='files', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, filename, relative_path)
    VALUES (new.id, new.filename, new.relative_path);
END;
CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, filename, relative_path)
    VALUES ('delete', old.id, old.filename, old.relative_path);
END;
CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, filename, relative_path)
    VALUES ('delete', old.id, old.filename, old.relative_path);
    INSERT INTO files_fts(rowid, filename, relative_path)
    VALUES (new.id, new.filename, new.relative_path);
END;
INSERT OR IGNORE INTO schema_version(version) VALUES (1);
"""

def connect(database_path: str | Path) -> sqlite3.Connection:
    path = Path(database_path).expanduser().resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA journal_mode = WAL")
    connection.executescript(SCHEMA)
    return connection
