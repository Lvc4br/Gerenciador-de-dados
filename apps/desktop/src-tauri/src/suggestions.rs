use rusqlite::{params, Connection};
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
           ON filename_suggestions(file_id);"
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
        } else if let Some(value) = token.strip_prefix("REV").or_else(|| token.strip_prefix("REVISION")).or_else(|| token.strip_prefix("REVISAO")) {
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
        "SELECT s.id,s.file_id,f.filename,f.relative_path,s.project_prefix,s.discipline_code,s.drawing_number,s.revision,s.confidence,s.status,s.created_at
         FROM filename_suggestions s JOIN files f ON f.id=s.file_id
         WHERE (?1='all' OR s.status=?1) AND (?2='' OR f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\' OR COALESCE(s.drawing_number,'') LIKE ?3 ESCAPE '\\')
         ORDER BY CASE s.status WHEN 'pending' THEN 0 ELSE 1 END, s.confidence DESC, f.filename COLLATE NOCASE
         LIMIT ?4 OFFSET ?5"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![status, search, pattern, limit.clamp(1,500), offset], |row| Ok(SuggestionRecord {
        id: row.get(0)?, file_id: row.get(1)?, filename: row.get(2)?, relative_path: row.get(3)?,
        project_prefix: row.get(4)?, discipline_code: row.get(5)?, drawing_number: row.get(6)?,
        revision: row.get(7)?, confidence: row.get(8)?, status: row.get(9)?, created_at: row.get(10)?,
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
