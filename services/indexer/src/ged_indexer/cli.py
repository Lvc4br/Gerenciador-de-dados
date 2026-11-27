from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from .database import connect
from .scanner import scan_folder, sha256_file
from .pdf import extract_pdf_text, PdfExtractionError
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
    extract = subparsers.add_parser("extract-pdf", help="Extrai texto de um PDF já indexado")
    extract.add_argument("file_id", type=int)
    search_content = subparsers.add_parser("search-content", help="Pesquisa texto extraído de PDFs")
    search_content.add_argument("query", type=str)
    search_content.add_argument("--limit", type=int, default=50)
    args = parser.parse_args()

    if args.command == "scan":
        with connect(args.database) as connection:
            result = scan_folder(connection, args.folder, args.max_file_size)
        print(json.dumps({"ok": True, **result}, ensure_ascii=False))
    elif args.command == "analyze-name":
        result = extract_filename_metadata(args.filename)
        print(json.dumps({"ok": True, **result}, ensure_ascii=False))
    elif args.command == "extract-pdf":
        try:
            with connect(args.database) as connection:
                row = connection.execute(
                    """SELECT f.id, f.filename, f.sha256, r.path, f.relative_path
                       FROM files f JOIN scan_roots r ON r.id=f.root_id WHERE f.id=?""",
                    (args.file_id,),
                ).fetchone()
                if row is None:
                    raise ValueError("Documento não encontrado no catálogo. Faça a varredura da pasta primeiro.")
                file_path = Path(row["path"]) / Path(row["relative_path"])
                if sha256_file(file_path) != row["sha256"]:
                    raise ValueError("O arquivo mudou desde a última varredura. Execute scan novamente antes de extrair.")
                result = extract_pdf_text(file_path)
                connection.execute(
                    """INSERT INTO extracted_document_text(
                           file_id,sha256,content,page_count,pages_with_text,character_count,parser,extracted_at
                       ) VALUES (?,?,?,?,?,?,?,CURRENT_TIMESTAMP)
                       ON CONFLICT(file_id) DO UPDATE SET sha256=excluded.sha256,
                         content=excluded.content,page_count=excluded.page_count,
                         pages_with_text=excluded.pages_with_text,character_count=excluded.character_count,
                         parser=excluded.parser,extracted_at=CURRENT_TIMESTAMP""",
                    (args.file_id,row["sha256"],result["text"],result["page_count"],
                     result["pages_with_text"],result["character_count"],result["parser"]),
                )
                connection.commit()
            print(json.dumps({"ok": True, "file_id": args.file_id, **{k:v for k,v in result.items() if k != "text"}}, ensure_ascii=False))
        except (PdfExtractionError, ValueError, OSError) as exc:
            parser.error(str(exc))
    elif args.command == "search-content":
        if args.limit < 1 or args.limit > 500:
            parser.error("--limit deve ficar entre 1 e 500")
        with connect(args.database) as connection:
            rows = connection.execute(
                """SELECT f.id AS file_id,f.filename,f.relative_path,r.path,
                          snippet(document_text_fts,0,'[',']',' … ',18) AS excerpt
                   FROM document_text_fts
                   JOIN files f ON f.id=document_text_fts.rowid
                   JOIN scan_roots r ON r.id=f.root_id
                   WHERE document_text_fts MATCH ?
                   ORDER BY rank LIMIT ?""",
                (args.query,args.limit),
            ).fetchall()
        print(json.dumps({"ok": True, "count": len(rows), "results": [
            {"file_id": row["file_id"], "filename": row["filename"],
             "path": str(Path(row["path"]) / Path(row["relative_path"])),
             "excerpt": row["excerpt"]} for row in rows
        ]}, ensure_ascii=False))

if __name__ == "__main__":
    main()
