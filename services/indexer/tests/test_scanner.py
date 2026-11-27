from pathlib import Path

from ged_indexer.database import connect
from ged_indexer.scanner import scan_folder

def test_scan_indexes_files_and_detects_duplicate_hashes(tmp_path: Path):
    root = tmp_path / "projects"
    root.mkdir()
    (root / "drawing-a.txt").write_text("same content", encoding="utf-8")
    (root / "drawing-b.txt").write_text("same content", encoding="utf-8")
    (root / "drawing-c.csv").write_text("id;title\n1;Bridge", encoding="utf-8")

    database = tmp_path / "ged.sqlite3"
    with connect(database) as connection:
        result = scan_folder(connection, root)
        rows = connection.execute("SELECT COUNT(*) AS n FROM files").fetchone()

    assert result["indexed"] == 3
    assert result["duplicate_hash_groups"] == 1
    assert rows["n"] == 3

def test_scan_does_not_follow_directory_symlinks(tmp_path: Path):
    root = tmp_path / "root"
    outside = tmp_path / "outside"
    root.mkdir()
    outside.mkdir()
    (outside / "secret.txt").write_text("outside", encoding="utf-8")
    link = root / "linked"
    try:
        link.symlink_to(outside, target_is_directory=True)
    except OSError:
        return

    with connect(tmp_path / "ged.sqlite3") as connection:
        result = scan_folder(connection, root)
        count = connection.execute("SELECT COUNT(*) AS n FROM files").fetchone()["n"]

    assert result["indexed"] == 0
    assert count == 0


def test_scan_persists_filename_suggestions_without_project_assignment(tmp_path: Path):
    root = tmp_path / "projects"
    root.mkdir()
    (root / "AB20A-EST-001_REV02.dwg").write_text("drawing", encoding="utf-8")

    with connect(tmp_path / "ged.sqlite3") as connection:
        scan_folder(connection, root)
        suggestion = connection.execute(
            """
            SELECT s.project_prefix, s.discipline_code, s.drawing_number,
                   s.revision, s.confidence, s.status
            FROM filename_suggestions s
            JOIN files f ON f.id=s.file_id
            WHERE f.filename='AB20A-EST-001_REV02.dwg'
            """
        ).fetchone()

        assert suggestion["project_prefix"] == "AB20A"
        assert suggestion["discipline_code"] == "EST"
        assert suggestion["drawing_number"] == "EST-001"
        assert suggestion["revision"] == "02"
        assert suggestion["confidence"] == 0.92
        assert suggestion["status"] == "pending"

        # A repeated scan with unchanged metadata must not reset human review.
        connection.execute(
            "UPDATE filename_suggestions SET status='accepted', reviewed_at=CURRENT_TIMESTAMP"
        )
        scan_folder(connection, root)
        status = connection.execute(
            "SELECT status FROM filename_suggestions"
        ).fetchone()["status"]

    assert status == "accepted"
