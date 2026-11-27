use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Serialize)]
pub struct SuggestionRecord {
    pub id: i64,
    pub file_id: i64,
    pub filename: String,
    pub relative_path: String,
    pub project_prefix: Option<String>,
    pub discipline_code: Option<String>,
    pub drawing_number: Option<String>,
    pub revision: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub applied: bool,
    pub titleblock_internal_number: Option<String>,
    pub titleblock_artesp_number: Option<String>,
    pub titleblock_title: Option<String>,
    pub created_at: String,
}

pub fn ensure_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS filename_suggestions (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
           project_prefix TEXT,
           discipline_code TEXT,
           drawing_number TEXT,
           revision TEXT,
           confidence REAL NOT NULL DEFAULT 0 CHECK(confidence >= 0 AND confidence <= 1),
           source TEXT NOT NULL DEFAULT 'filename',
           parser_version TEXT NOT NULL DEFAULT '1',
           status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','accepted','rejected')),
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           reviewed_at TEXT,
           UNIQUE(file_id, parser_version)
         );
         CREATE INDEX IF NOT EXISTS idx_filename_suggestions_status
           ON filename_suggestions(status, confidence DESC);
         CREATE INDEX IF NOT EXISTS idx_filename_suggestions_file
           ON filename_suggestions(file_id);
         CREATE TABLE IF NOT EXISTS confirmed_document_metadata (
           file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
           project_prefix TEXT,
           discipline_code TEXT,
           drawing_number TEXT,
           revision TEXT,
           source_suggestion_id INTEGER REFERENCES filename_suggestions(id),
           confirmed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS metadata_audit_log (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
           suggestion_id INTEGER REFERENCES filename_suggestions(id),
           action TEXT NOT NULL,
           old_values_json TEXT,
           new_values_json TEXT NOT NULL,
           actor TEXT NOT NULL DEFAULT 'local-user',
           occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE INDEX IF NOT EXISTS idx_metadata_audit_file ON metadata_audit_log(file_id, occurred_at DESC);"
    ).map_err(|e| e.to_string())
}

fn parse_filename(filename: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>, f64) {
    let stem = filename.rsplit_once('.').map(|v| v.0).unwrap_or(filename);
    let upper = stem.to_ascii_uppercase();
    let tokens: Vec<&str> = upper.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|v| !v.is_empty()).collect();
    let project = tokens.first().filter(|v| {
        let has_letter = v.chars().any(|c| c.is_ascii_alphabetic());
        let has_digit = v.chars().any(|c| c.is_ascii_digit());
        has_letter && has_digit && v.len() <= 10
    }).map(|v| (*v).to_string());

    let mut discipline: Option<String> = None;
    let mut drawing: Option<String> = None;
    let mut revision: Option<String> = None;
    for (i, token) in tokens.iter().enumerate() {
        let token = *token;
        if token == "REV" || token == "REVISION" || token == "REVISAO" || token == "R" {
            if let Some(next) = tokens.get(i + 1) {
                if next.len() <= 3 && (next.chars().all(|c| c.is_ascii_digit()) || (next.len() == 1 && next.chars().all(|c| c.is_ascii_alphabetic()))) {
                    revision = Some((*next).to_string());
                }
            }
        } else if let Some(value) = token.strip_prefix("REVISION").or_else(|| token.strip_prefix("REVISAO")).or_else(|| token.strip_prefix("REV")) {
            if !value.is_empty() && value.len() <= 3 && value.chars().all(|c| c.is_ascii_alphanumeric()) {
                revision = Some(value.to_string());
            }
        } else if let Some(value) = token.strip_prefix('R') {
            if !value.is_empty() && value.len() <= 3 && value.chars().all(|c| c.is_ascii_digit()) {
                revision = Some(value.to_string());
            }
        }
        if i + 1 < tokens.len() && (2..=5).contains(&token.len())
            && token.chars().all(|c| c.is_ascii_alphabetic())
            && tokens[i + 1].len() >= 2 && tokens[i + 1].len() <= 5
            && tokens[i + 1].chars().all(|c| c.is_ascii_digit()) {
            discipline = Some(token.to_string());
            drawing = Some(format!("{}-{}", token, tokens[i + 1]));
        }
    }
    let confidence = if drawing.is_some() && revision.is_some() { 0.92 }
        else if drawing.is_some() { 0.78 }
        else if revision.is_some() { 0.45 } else { 0.0 };
    (project, discipline, drawing, revision, confidence)
}

pub fn analyze_and_store(connection: &Connection, file_id: i64, filename: &str) -> Result<(), String> {
    let (project, discipline, drawing, revision, confidence) = parse_filename(filename);
    if confidence <= 0.0 { return Ok(()); }
    connection.execute(
        "INSERT INTO filename_suggestions(file_id, project_prefix, discipline_code, drawing_number, revision, confidence)
         VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(file_id, parser_version) DO UPDATE SET
           project_prefix=excluded.project_prefix,
           discipline_code=excluded.discipline_code,
           drawing_number=excluded.drawing_number,
           revision=excluded.revision,
           confidence=excluded.confidence,
           status=CASE
             WHEN filename_suggestions.project_prefix IS excluded.project_prefix
              AND filename_suggestions.discipline_code IS excluded.discipline_code
              AND filename_suggestions.drawing_number IS excluded.drawing_number
              AND filename_suggestions.revision IS excluded.revision
             THEN filename_suggestions.status ELSE 'pending' END,
           reviewed_at=CASE
             WHEN filename_suggestions.project_prefix IS excluded.project_prefix
              AND filename_suggestions.discipline_code IS excluded.discipline_code
              AND filename_suggestions.drawing_number IS excluded.drawing_number
              AND filename_suggestions.revision IS excluded.revision
             THEN filename_suggestions.reviewed_at ELSE NULL END",
        params![file_id, project, discipline, drawing, revision, confidence]
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list(connection: &Connection, status: &str, search: &str, limit: u32, offset: u32) -> Result<(Vec<SuggestionRecord>, u64), String> {
    let status = match status { "pending" | "accepted" | "rejected" => status, _ => "all" };
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM filename_suggestions s JOIN files f ON f.id=s.file_id
         WHERE (?1='all' OR s.status=?1) AND (?2='' OR f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\' OR COALESCE(s.drawing_number,'') LIKE ?3 ESCAPE '\\')",
        params![status, search, pattern], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let mut statement = connection.prepare(
        "SELECT s.id,s.file_id,f.filename,f.relative_path,s.project_prefix,s.discipline_code,s.drawing_number,s.revision,s.confidence,s.status,EXISTS(SELECT 1 FROM confirmed_document_metadata m WHERE m.source_suggestion_id=s.id),
         (SELECT dm.internal_number FROM drawing_metadata dm WHERE lower(trim(dm.filename))=lower(trim(f.filename)) ORDER BY dm.imported_at DESC LIMIT 1),
         (SELECT dm.artesp_number FROM drawing_metadata dm WHERE lower(trim(dm.filename))=lower(trim(f.filename)) ORDER BY dm.imported_at DESC LIMIT 1),
         (SELECT dm.candidate_title FROM drawing_metadata dm WHERE lower(trim(dm.filename))=lower(trim(f.filename)) ORDER BY dm.imported_at DESC LIMIT 1),s.created_at
         FROM filename_suggestions s JOIN files f ON f.id=s.file_id
         WHERE (?1='all' OR s.status=?1) AND (?2='' OR f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\' OR COALESCE(s.drawing_number,'') LIKE ?3 ESCAPE '\\')
         ORDER BY CASE s.status WHEN 'pending' THEN 0 ELSE 1 END, s.confidence DESC, f.filename COLLATE NOCASE
         LIMIT ?4 OFFSET ?5"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![status, search, pattern, limit.clamp(1,500), offset], |row| Ok(SuggestionRecord {
        id: row.get(0)?, file_id: row.get(1)?, filename: row.get(2)?, relative_path: row.get(3)?,
        project_prefix: row.get(4)?, discipline_code: row.get(5)?, drawing_number: row.get(6)?,
        revision: row.get(7)?, confidence: row.get(8)?, status: row.get(9)?, applied: row.get(10)?,
        titleblock_internal_number: row.get(11)?, titleblock_artesp_number: row.get(12)?, titleblock_title: row.get(13)?, created_at: row.get(14)?,
    })).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok((items, total))
}

pub fn review(connection: &Connection, id: i64, status: &str) -> Result<(), String> {
    if status != "accepted" && status != "rejected" {
        return Err("A decisão deve ser accepted ou rejected.".into());
    }
    let changed = connection.execute(
        "UPDATE filename_suggestions SET status=?1, reviewed_at=CURRENT_TIMESTAMP WHERE id=?2 AND status='pending'",
        params![status, id]
    ).map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("A sugestão não existe ou já foi revisada.".into());
    }
    Ok(())
}


pub fn apply_accepted(connection: &Connection, id: i64) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    let values: (i64, Option<String>, Option<String>, Option<String>, Option<String>, String) =
        transaction.query_row(
            "SELECT file_id,project_prefix,discipline_code,drawing_number,revision,status
             FROM filename_suggestions WHERE id=?1",
            params![id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))
        ).map_err(|e| e.to_string())?;
    if values.5 != "accepted" {
        return Err("Somente sugestões aceitas podem ser aplicadas.".into());
    }
    let already_applied: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM confirmed_document_metadata WHERE source_suggestion_id=?1)",
        params![id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if already_applied {
        transaction.commit().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let old: Option<String> = transaction.query_row(
        "SELECT json_object(
           'project_prefix',project_prefix,'discipline_code',discipline_code,
           'drawing_number',drawing_number,'revision',revision)
         FROM confirmed_document_metadata WHERE file_id=?1",
        params![values.0], |row| row.get(0)
    ).optional().map_err(|e| e.to_string())?;
    let new_json = serde_json::json!({
        "project_prefix": values.1, "discipline_code": values.2,
        "drawing_number": values.3, "revision": values.4
    }).to_string();
    transaction.execute(
        "INSERT INTO confirmed_document_metadata(file_id,project_prefix,discipline_code,drawing_number,revision,source_suggestion_id,confirmed_at)
         VALUES (?1,?2,?3,?4,?5,?6,CURRENT_TIMESTAMP)
         ON CONFLICT(file_id) DO UPDATE SET
           project_prefix=excluded.project_prefix, discipline_code=excluded.discipline_code,
           drawing_number=excluded.drawing_number, revision=excluded.revision,
           source_suggestion_id=excluded.source_suggestion_id, confirmed_at=CURRENT_TIMESTAMP",
        params![values.0,values.1,values.2,values.3,values.4,id]
    ).map_err(|e| e.to_string())?;
    transaction.execute(
        "INSERT INTO metadata_audit_log(file_id,suggestion_id,action,old_values_json,new_values_json)
         VALUES (?1,?2,?3,?4,?5)",
        params![values.0,id,if old.is_some() {"metadata_updated"} else {"metadata_created"},old,new_json]
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

    #[test]
    fn applies_accepted_suggestion_and_writes_audit_record() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE files(id INTEGER PRIMARY KEY, filename TEXT);").unwrap();
        ensure_schema(&connection).unwrap();
        connection.execute("INSERT INTO files(id,filename) VALUES(1,'AB20A-EST-001_REV02.dwg')", []).unwrap();
        analyze_and_store(&connection, 1, "AB20A-EST-001_REV02.dwg").unwrap();
        review(&connection, 1, "accepted").unwrap();
        apply_accepted(&connection, 1).unwrap();
        let drawing: String = connection.query_row("SELECT drawing_number FROM confirmed_document_metadata WHERE file_id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(drawing, "EST-001");
        let audit_count: i64 = connection.query_row("SELECT COUNT(*) FROM metadata_audit_log WHERE file_id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(audit_count, 1);
        assert!(apply_accepted(&connection, 999).is_err());
    }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_project_drawing_and_revision_without_confusing_rev_as_discipline() {
        let (project, discipline, drawing, revision, confidence) = parse_filename("AB20A-EST-001_REV02.dwg");
        assert_eq!(project.as_deref(), Some("AB20A"));
        assert_eq!(discipline.as_deref(), Some("EST"));
        assert_eq!(drawing.as_deref(), Some("EST-001"));
        assert_eq!(revision.as_deref(), Some("02"));
        assert_eq!(confidence, 0.92);
    }

    #[test]
    fn parses_long_revision_marker() {
        let (_, _, drawing, revision, _) = parse_filename("AB20A-DRE-014_REVISION03.pdf");
        assert_eq!(drawing.as_deref(), Some("DRE-014"));
        assert_eq!(revision.as_deref(), Some("03"));
    }

    #[test]
    fn review_is_one_way_from_pending() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE files(id INTEGER PRIMARY KEY, filename TEXT);").unwrap();
        ensure_schema(&connection).unwrap();
        connection.execute("INSERT INTO files(id,filename) VALUES(1,'AB20A-EST-001_REV02.dwg')", []).unwrap();
        analyze_and_store(&connection, 1, "AB20A-EST-001_REV02.dwg").unwrap();
        review(&connection, 1, "accepted").unwrap();
        assert!(review(&connection, 1, "rejected").is_err());
    }
}
