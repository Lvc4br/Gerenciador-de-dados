from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from .database import connect
from .scanner import scan_folder
from .naming import extract_filename_metadata

def default_database_path() -> Path:
    base = Path(os.environ.get("LOCALAPPDATA", Path.home() / ".local" / "share"))
    return base / "GED" / "ged.sqlite3"

def main() -> None:
    parser = argparse.ArgumentParser(prog="ged-indexer")
    parser.add_argument("--database", type=Path, default=default_database_path())
    subparsers = parser.add_subparsers(dest="command", required=True)
    scan = subparsers.add_parser("scan", help="Indexa uma pasta sem alterar os originais")
    scan.add_argument("folder", type=Path)
    scan.add_argument("--max-file-size", type=int, default=2 * 1024 * 1024 * 1024)
    analyze = subparsers.add_parser(
        "analyze-name", help="Sugere metadados a partir do nome, sem alterar arquivos"
    )
    analyze.add_argument("filename", type=str)
    args = parser.parse_args()

    if args.command == "scan":
        with connect(args.database) as connection:
            result = scan_folder(connection, args.folder, args.max_file_size)
        print(json.dumps({"ok": True, **result}, ensure_ascii=False))
    elif args.command == "analyze-name":
        result = extract_filename_metadata(args.filename)
        print(json.dumps({"ok": True, **result}, ensure_ascii=False))

if __name__ == "__main__":
    main()
