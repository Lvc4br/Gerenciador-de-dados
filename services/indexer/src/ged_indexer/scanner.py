from __future__ import annotations

import hashlib
import os
from pathlib import Path
import sqlite3
from datetime import datetime, timezone

from .naming import extract_filename_metadata

CHUNK_SIZE = 1024 * 1024
DEFAULT_MAX_FILE_SIZE = 2 * 1024 * 1024 * 1024

def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(CHUNK_SIZE), b""):
            digest.update(chunk)
    return digest.hexdigest()

def scan_folder(
    connection: sqlite3.Connection,
    folder: str | Path,
    max_file_size: int = DEFAULT_MAX_FILE_SIZE,
) -> dict[str, int | str]:
    root = Path(folder).expanduser().resolve(strict=True)
    if not root.is_dir():
        raise ValueError("O caminho informado não é uma pasta.")

    cursor = connection.execute(
        "INSERT OR IGNORE INTO scan_roots(path) VALUES (?)", (str(root),)
    )
    connection.commit()
    row = connection.execute(
        "SELECT id FROM scan_roots WHERE path = ?", (str(root),)
    ).fetchone()
    root_id = int(row["id"])

    indexed = skipped = errors = 0
    for current, dirs, filenames in os.walk(root, followlinks=False):
        current_path = Path(current)
        dirs[:] = [
            name for name in dirs
            if not (current_path / name).is_symlink()
        ]
        for filename in filenames:
            path = current_path / filename
            try:
                if path.is_symlink() or not path.is_file():
                    skipped += 1
                    continue
                stat = path.stat()
                if stat.st_size > max_file_size:
                    skipped += 1
                    continue
                relative = path.relative_to(root).as_posix()
                digest = sha256_file(path)
                modified = datetime.fromtimestamp(
                    stat.st_mtime, tz=timezone.utc
                ).isoformat()
                connection.execute(
                    """
                    INSERT INTO files(
                        root_id, relative_path, filename, extension,
                        byte_size, modified_at, sha256, status
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, 'indexed')
                    ON CONFLICT(root_id, relative_path) DO UPDATE SET
                        filename=excluded.filename,
                        extension=excluded.extension,
                        byte_size=excluded.byte_size,
                        modified_at=excluded.modified_at,
                        sha256=excluded.sha256,
                        status='indexed',
                        indexed_at=CURRENT_TIMESTAMP
                    """,
                    (
                        root_id, relative, filename, path.suffix.lower(),
                        stat.st_size, modified, digest,
                    ),
                )
                file_row = connection.execute(
                    "SELECT id FROM files WHERE root_id=? AND relative_path=?",
                    (root_id, relative),
                ).fetchone()
                suggestion = extract_filename_metadata(filename)
                connection.execute(
                    """
                    INSERT INTO filename_suggestions(
                        file_id, project_prefix, discipline_code, drawing_number,
                        revision, confidence, source, parser_version
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, '1')
                    ON CONFLICT(file_id, parser_version) DO UPDATE SET
                        project_prefix=excluded.project_prefix,
                        discipline_code=excluded.discipline_code,
                        drawing_number=excluded.drawing_number,
                        revision=excluded.revision,
                        confidence=excluded.confidence,
                        source=excluded.source,
                        status='pending',
                        reviewed_at=NULL,
                        created_at=CURRENT_TIMESTAMP
                    WHERE filename_suggestions.project_prefix IS NOT excluded.project_prefix
                       OR filename_suggestions.discipline_code IS NOT excluded.discipline_code
                       OR filename_suggestions.drawing_number IS NOT excluded.drawing_number
                       OR filename_suggestions.revision IS NOT excluded.revision
                       OR filename_suggestions.confidence != excluded.confidence
                    """,
                    (
                        int(file_row["id"]),
                        suggestion["project_prefix"],
                        suggestion["discipline_code"],
                        suggestion["drawing_number"],
                        suggestion["revision"],
                        suggestion["confidence"],
                        suggestion["source"],
                    ),
                )
                indexed += 1
            except (OSError, PermissionError):
                errors += 1
    connection.commit()

    duplicates = connection.execute(
        """
        SELECT COUNT(*) AS amount FROM (
            SELECT sha256 FROM files GROUP BY sha256 HAVING COUNT(*) > 1
        )
        """
    ).fetchone()["amount"]
    return {
        "root": str(root),
        "indexed": indexed,
        "skipped": skipped,
        "errors": errors,
        "duplicate_hash_groups": int(duplicates),
    }
