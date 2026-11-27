from ged_indexer.database import connect


def test_extracted_pdf_text_is_searchable_and_updates_fts(tmp_path):
    database = tmp_path / "ged.sqlite3"
    with connect(database) as connection:
        connection.execute("INSERT INTO scan_roots(path) VALUES (?)", (str(tmp_path),))
        root_id = connection.execute("SELECT id FROM scan_roots").fetchone()["id"]
        connection.execute(
            """INSERT INTO files(root_id,relative_path,filename,extension,byte_size,modified_at,sha256)
               VALUES (?,?,?,?,?,?,?)""",
            (root_id,"projeto/desenho.pdf","desenho.pdf",".pdf",123,"2026-11-27T10:00:00+00:00","abc"),
        )
        file_id = connection.execute("SELECT id FROM files").fetchone()["id"]
        connection.execute(
            """INSERT INTO extracted_document_text(file_id,sha256,content,page_count,pages_with_text,character_count,parser)
               VALUES (?,?,?,?,?,?,?)""",
            (file_id,"abc","Número do desenho EST-001 revisão 02",1,1,38,"pypdf"),
        )
        result = connection.execute(
            "SELECT rowid FROM document_text_fts WHERE document_text_fts MATCH ?",
            ('"EST-001"',),
        ).fetchone()
        assert result is not None
        connection.execute("UPDATE extracted_document_text SET content=? WHERE file_id=?", ("Memorial descritivo rodovia",file_id))
        assert connection.execute(
            "SELECT rowid FROM document_text_fts WHERE document_text_fts MATCH ?", ('"EST-001"',)
        ).fetchone() is None
        assert connection.execute(
            "SELECT rowid FROM document_text_fts WHERE document_text_fts MATCH ?", ("rodovia",)
        ).fetchone() is not None
