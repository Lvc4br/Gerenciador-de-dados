from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path

from .database import connect
from .scanner import scan_folder, sha256_file
from .pdf import extract_pdf_text, PdfExtractionError
from .naming import extract_filename_metadata

def build_content_fts_query(query: str) -> str:
    terms = [term for term in re.findall(r"[^\\W_]+", query, flags=re.UNICODE) if len(term) >= 2]
    if not terms:
        raise ValueError("Informe ao menos um termo com dois ou mais caracteres.")
    return " AND ".join(f'"{term.replace(chr(34), chr(34) * 2)}"*' for term in terms)

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
    extract_many = subparsers.add_parser("extract-pdfs", help="Extrai em lote PDFs ainda não processados")
    extract_many.add_argument("--limit", type=int, default=100)
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
    elif args.command == "extract-pdfs":
        if args.limit < 1 or args.limit > 500:
            parser.error("--limit deve ficar entre 1 e 500")
        processed = skipped = errors = 0
        error_details = []
        with connect(args.database) as connection:
            rows = connection.execute(
                """SELECT f.id,f.filename,f.sha256,r.path,f.relative_path
                   FROM files f JOIN scan_roots r ON r.id=f.root_id
                   LEFT JOIN extracted_document_text x ON x.file_id=f.id
                   WHERE lower(f.extension)='.pdf'
                     AND (x.file_id IS NULL OR x.sha256 != f.sha256)
                   ORDER BY f.id LIMIT ?""",
                (args.limit,),
            ).fetchall()
            for row in rows:
                file_path = Path(row["path"]) / Path(row["relative_path"])
                try:
                    if sha256_file(file_path) != row["sha256"]:
                        raise ValueError("Arquivo mudou desde a varredura; execute scan novamente.")
                    result = extract_pdf_text(file_path)
                    connection.execute(
                        """INSERT INTO extracted_document_text(
                               file_id,sha256,content,page_count,pages_with_text,character_count,parser
                           ) VALUES (?,?,?,?,?,?,?)
                           ON CONFLICT(file_id) DO UPDATE SET sha256=excluded.sha256,
                             content=excluded.content,page_count=excluded.page_count,
                             pages_with_text=excluded.pages_with_text,character_count=excluded.character_count,
                             parser=excluded.parser,extracted_at=CURRENT_TIMESTAMP""",
                        (row["id"],row["sha256"],result["text"],result["page_count"],
                         result["pages_with_text"],result["character_count"],result["parser"]),
                    )
                    processed += 1
                except (PdfExtractionError, ValueError, OSError) as exc:
                    errors += 1
                    error_details.append({"file_id": row["id"], "filename": row["filename"], "error": str(exc)})
            connection.commit()
        print(json.dumps({"ok": errors == 0, "processed": processed, "skipped": skipped,
                          "errors": errors, "error_details": error_details}, ensure_ascii=False))
    elif args.command == "search-content":
        if args.limit < 1 or args.limit > 500:
            parser.error("--limit deve ficar entre 1 e 500")
        try:
            fts_query = build_content_fts_query(args.query)
        except ValueError as exc:
            parser.error(str(exc))
        with connect(args.database) as connection:
            rows = connection.execute(
                """SELECT f.id AS file_id,f.filename,f.relative_path,r.path,
                          snippet(document_text_fts,0,'[',']',' … ',18) AS excerpt
                   FROM document_text_fts
                   JOIN files f ON f.id=document_text_fts.rowid
                   JOIN scan_roots r ON r.id=f.root_id
                   WHERE document_text_fts MATCH ?
                   ORDER BY rank LIMIT ?""",
                (fts_query,args.limit),
            ).fetchall()
        print(json.dumps({"ok": True, "count": len(rows), "results": [
            {"file_id": row["file_id"], "filename": row["filename"],
             "path": str(Path(row["path"]) / Path(row["relative_path"])),
             "excerpt": row["excerpt"]} for row in rows
        ]}, ensure_ascii=False))

if __name__ == "__main__":
    main()
