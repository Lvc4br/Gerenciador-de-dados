use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
pub struct ProjectRecord {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub client: String,
    pub description: String,
    pub status: String,
    pub root_count: u64,
    pub file_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ProjectRootRecord {
    pub id: i64,
    pub path: String,
    pub added_at: String,
    pub linked: bool,
}

pub fn ensure_schema(connection: &Connection) -> Result<(), String> {
    let applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=4", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if applied > 0 { return Ok(()); }

    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           code TEXT NOT NULL COLLATE NOCASE UNIQUE,
           name TEXT NOT NULL,
           client TEXT NOT NULL DEFAULT '',
           description TEXT NOT NULL DEFAULT '',
           status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
           created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS project_roots (
           project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
           root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,
           linked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           PRIMARY KEY(project_id, root_id)
         );
         CREATE INDEX IF NOT EXISTS idx_projects_status_code ON projects(status, code);
         CREATE INDEX IF NOT EXISTS idx_project_roots_root ON project_roots(root_id);
         CREATE TABLE IF NOT EXISTS project_disciplines (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
           code TEXT NOT NULL,
           name TEXT NOT NULL,
           sort_order INTEGER NOT NULL DEFAULT 0,
           UNIQUE(project_id, code COLLATE NOCASE)
         );
         INSERT INTO schema_migrations(version, name)
         VALUES (4, 'offline_project_catalog');"
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

pub fn list(connection: &Connection, search: &str, status: &str, limit: u32, offset: u32) -> Result<(Vec<ProjectRecord>, u64), String> {
    let search = search.trim();
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let status_filter = match status { "active" | "archived" => Some(status), _ => None };
    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM projects p
         WHERE (?1 = '' OR p.code LIKE ?2 ESCAPE '\\' OR p.name LIKE ?2 ESCAPE '\\' OR p.client LIKE ?2 ESCAPE '\\')
           AND (?3 IS NULL OR p.status = ?3)",
        params![search, pattern, status_filter], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let mut statement = connection.prepare(
        "SELECT p.id,p.code,p.name,p.client,p.description,p.status,
           (SELECT COUNT(*) FROM project_roots pr WHERE pr.project_id=p.id),
           (SELECT COUNT(*) FROM files f JOIN project_roots pr ON pr.root_id=f.root_id
             WHERE pr.project_id=p.id AND f.status='indexed'),
           p.created_at,p.updated_at
         FROM projects p
         WHERE (?1 = '' OR p.code LIKE ?2 ESCAPE '\\' OR p.name LIKE ?2 ESCAPE '\\' OR p.client LIKE ?2 ESCAPE '\\')
           AND (?3 IS NULL OR p.status = ?3)
         ORDER BY CASE p.status WHEN 'active' THEN 0 ELSE 1 END, p.code COLLATE NOCASE
         LIMIT ?4 OFFSET ?5"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![search, pattern, status_filter, limit.clamp(1, 200), offset], |row| {
        Ok(ProjectRecord {
            id: row.get(0)?, code: row.get(1)?, name: row.get(2)?, client: row.get(3)?,
            description: row.get(4)?, status: row.get(5)?, root_count: row.get(6)?,
            file_count: row.get(7)?, created_at: row.get(8)?, updated_at: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok((items, total))
}

pub fn create(connection: &Connection, code: &str, name: &str, client: &str, description: &str) -> Result<i64, String> {
    let code = code.trim().to_uppercase();
    let name = name.trim();
    if code.is_empty() { return Err("Informe o código do projeto.".into()); }
    if name.is_empty() { return Err("Informe o nome do projeto.".into()); }
    if code.chars().count() > 40 { return Err("O código deve ter no máximo 40 caracteres.".into()); }
    if name.chars().count() > 180 { return Err("O nome deve ter no máximo 180 caracteres.".into()); }
    connection.execute(
        "INSERT INTO projects(code,name,client,description) VALUES (?1,?2,?3,?4)",
        params![code, name, client.trim(), description.trim()]
    ).map_err(|error| {
        if error.to_string().contains("UNIQUE constraint failed") {
            "Já existe um projeto com esse código.".to_string()
        } else { error.to_string() }
    })?;
    Ok(connection.last_insert_rowid())
}

pub fn update(connection: &Connection, id: i64, code: &str, name: &str, client: &str, description: &str) -> Result<(), String> {
    let code = code.trim().to_uppercase();
    let name = name.trim();
    if code.is_empty() || name.is_empty() { return Err("Código e nome são obrigatórios.".into()); }
    let changed = connection.execute(
        "UPDATE projects SET code=?1,name=?2,client=?3,description=?4,updated_at=CURRENT_TIMESTAMP WHERE id=?5",
        params![code,name,client.trim(),description.trim(),id]
    ).map_err(|error| {
        if error.to_string().contains("UNIQUE constraint failed") {
            "Já existe outro projeto com esse código.".to_string()
        } else { error.to_string() }
    })?;
    if changed == 0 { return Err("Projeto não encontrado.".into()); }
    Ok(())
}

pub fn set_archived(connection: &Connection, id: i64, archived: bool) -> Result<(), String> {
    let status = if archived { "archived" } else { "active" };
    let changed = connection.execute(
        "UPDATE projects SET status=?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2",
        params![status,id]
    ).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("Projeto não encontrado.".into()); }
    Ok(())
}

pub fn list_roots(connection: &Connection, project_id: i64) -> Result<Vec<ProjectRootRecord>, String> {
    let mut statement = connection.prepare(
        "SELECT r.id,r.path,r.added_at,CASE WHEN pr.project_id IS NULL THEN 0 ELSE 1 END
         FROM scan_roots r LEFT JOIN project_roots pr ON pr.root_id=r.id AND pr.project_id=?1
         ORDER BY r.path COLLATE NOCASE"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![project_id], |row| Ok(ProjectRootRecord {
        id: row.get(0)?, path: row.get(1)?, added_at: row.get(2)?, linked: row.get::<_,i64>(3)? == 1
    })).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok(items)
}

pub fn link_root(connection: &Connection, project_id: i64, root_id: i64, linked: bool) -> Result<(), String> {
    let project_exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)", params![project_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let root_exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots WHERE id=?1)", params![root_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !project_exists { return Err("Projeto não encontrado.".into()); }
    if !root_exists { return Err("Pasta indexada não encontrada.".into()); }
    if linked {
        connection.execute("INSERT OR IGNORE INTO project_roots(project_id,root_id) VALUES (?1,?2)", params![project_id,root_id]).map_err(|e| e.to_string())?;
    } else {
        connection.execute("DELETE FROM project_roots WHERE project_id=?1 AND root_id=?2", params![project_id,root_id]).map_err(|e| e.to_string())?;
    }
    connection.execute("UPDATE projects SET updated_at=CURRENT_TIMESTAMP WHERE id=?1", params![project_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE scan_roots(id INTEGER PRIMARY KEY,path TEXT NOT NULL UNIQUE,added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE files(id INTEGER PRIMARY KEY,root_id INTEGER NOT NULL,status TEXT NOT NULL);"
        ).unwrap();
        ensure_schema(&connection).unwrap();
        connection
    }

    #[test]
    fn project_code_is_case_insensitive_and_normalized() {
        let connection = db();
        create(&connection," ab20a ","Projeto AB20A","","").unwrap();
        let duplicate = create(&connection,"Ab20A","Outro","","").unwrap_err();
        assert!(duplicate.contains("Já existe"));
        let code: String = connection.query_row("SELECT code FROM projects",[],|r|r.get(0)).unwrap();
        assert_eq!(code,"AB20A");
    }

    #[test]
    fn project_can_share_root_without_copying_files() {
        let connection = db();
        create(&connection,"P1","Projeto 1","","").unwrap();
        create(&connection,"P2","Projeto 2","","").unwrap();
        connection.execute("INSERT INTO scan_roots(path) VALUES ('C:/GED/AB20A')",[]).unwrap();
        let root_id: i64 = connection.query_row("SELECT id FROM scan_roots",[],|r|r.get(0)).unwrap();
        link_root(&connection,1,root_id,true).unwrap();
        link_root(&connection,2,root_id,true).unwrap();
        let links: i64 = connection.query_row("SELECT COUNT(*) FROM project_roots",[],|r|r.get(0)).unwrap();
        assert_eq!(links,2);
        let roots: i64 = connection.query_row("SELECT COUNT(*) FROM scan_roots",[],|r|r.get(0)).unwrap();
        assert_eq!(roots,1);
    }

    #[test]
    fn archive_preserves_project_and_links() {
        let connection = db();
        create(&connection,"P1","Projeto 1","","").unwrap();
        set_archived(&connection,1,true).unwrap();
        let status: String = connection.query_row("SELECT status FROM projects WHERE id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(status,"archived");
    }
}


#[derive(Serialize)]
pub struct ProjectDisciplineRecord {
    pub id: i64,
    pub project_id: i64,
    pub code: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Serialize)]
pub struct ProjectFileRecord {
    pub id: i64,
    pub filename: String,
    pub relative_path: String,
    pub extension: String,
    pub byte_size: i64,
    pub modified_at: String,
    pub sha256: String,
    pub root_path: String,
    pub status: String,
    pub linked: bool,
    pub discipline_id: Option<i64>,
    pub discipline_code: Option<String>,
    pub discipline_name: Option<String>,
}

pub fn ensure_relations_schema(connection: &Connection) -> Result<(), String> {
    let applied: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=5", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if applied > 0 { return Ok(()); }
    let transaction = connection.unchecked_transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_file_links (
           project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
           file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
           discipline_id INTEGER REFERENCES project_disciplines(id) ON DELETE SET NULL,
           source TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','rule')),
           confidence REAL,
           linked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
           PRIMARY KEY(project_id,file_id)
         );
         CREATE INDEX IF NOT EXISTS idx_project_file_links_discipline
           ON project_file_links(project_id,discipline_id);
         INSERT INTO schema_migrations(version,name)
           VALUES (5,'project_document_relationships');"
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

pub fn list_disciplines(connection: &Connection, project_id: i64) -> Result<Vec<ProjectDisciplineRecord>, String> {
    let mut statement = connection.prepare(
        "SELECT id,project_id,code,name,sort_order FROM project_disciplines
         WHERE project_id=?1 ORDER BY sort_order,name COLLATE NOCASE"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![project_id], |row| Ok(ProjectDisciplineRecord {
        id: row.get(0)?, project_id: row.get(1)?, code: row.get(2)?,
        name: row.get(3)?, sort_order: row.get(4)?
    })).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok(items)
}

pub fn save_discipline(connection: &Connection, project_id: i64, id: Option<i64>, code: &str, name: &str, sort_order: i64) -> Result<i64, String> {
    let code = code.trim().to_uppercase();
    let name = name.trim();
    if code.is_empty() || name.is_empty() {
        return Err("Código e nome da disciplina são obrigatórios.".into());
    }
    if code.chars().count() > 30 || name.chars().count() > 100 {
        return Err("Código ou nome da disciplina excede o limite permitido.".into());
    }
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)", params![project_id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if !exists { return Err("Projeto não encontrado.".into()); }
    if let Some(id) = id {
        let changed = connection.execute(
            "UPDATE project_disciplines SET code=?1,name=?2,sort_order=?3 WHERE id=?4 AND project_id=?5",
            params![code,name,sort_order,id,project_id]
        ).map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "Já existe uma disciplina com esse código neste projeto.".to_string()
            } else { error.to_string() }
        })?;
        if changed == 0 { return Err("Disciplina não encontrada neste projeto.".into()); }
        Ok(id)
    } else {
        connection.execute(
            "INSERT INTO project_disciplines(project_id,code,name,sort_order) VALUES (?1,?2,?3,?4)",
            params![project_id,code,name,sort_order]
        ).map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "Já existe uma disciplina com esse código neste projeto.".to_string()
            } else { error.to_string() }
        })?;
        Ok(connection.last_insert_rowid())
    }
}

pub fn delete_discipline(connection: &Connection, project_id: i64, discipline_id: i64) -> Result<(), String> {
    let changed = connection.execute(
        "DELETE FROM project_disciplines WHERE id=?1 AND project_id=?2",
        params![discipline_id,project_id]
    ).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("Disciplina não encontrada neste projeto.".into()); }
    Ok(())
}

pub fn list_project_files(connection: &Connection, project_id: i64, search: &str, discipline_id: Option<i64>, limit: u32, offset: u32) -> Result<(Vec<ProjectFileRecord>,u64),String> {
    let search = search.trim();
    let pattern = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
    let total: u64 = connection.query_row(
        "SELECT COUNT(*) FROM files f
         LEFT JOIN project_roots pr ON pr.root_id=f.root_id AND pr.project_id=?1
         LEFT JOIN project_file_links pfl ON pfl.file_id=f.id AND pfl.project_id=?1
         WHERE f.status='indexed' AND (pr.project_id IS NOT NULL OR pfl.project_id IS NOT NULL)
           AND (?2='' OR f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')
           AND (?4 IS NULL OR pfl.discipline_id=?4)",
        params![project_id,search,pattern,discipline_id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let mut statement = connection.prepare(
        "SELECT f.id,f.filename,f.relative_path,f.extension,f.byte_size,f.modified_at,f.sha256,r.path,f.status,
           CASE WHEN pfl.project_id IS NULL THEN 0 ELSE 1 END,pfl.discipline_id,d.code,d.name
         FROM files f JOIN scan_roots r ON r.id=f.root_id
         LEFT JOIN project_roots pr ON pr.root_id=f.root_id AND pr.project_id=?1
         LEFT JOIN project_file_links pfl ON pfl.file_id=f.id AND pfl.project_id=?1
         LEFT JOIN project_disciplines d ON d.id=pfl.discipline_id
         WHERE f.status='indexed' AND (pr.project_id IS NOT NULL OR pfl.project_id IS NOT NULL)
           AND (?2='' OR f.filename LIKE ?3 ESCAPE '\\' OR f.relative_path LIKE ?3 ESCAPE '\\')
           AND (?4 IS NULL OR pfl.discipline_id=?4)
         ORDER BY f.relative_path COLLATE NOCASE LIMIT ?5 OFFSET ?6"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(
        params![project_id,search,pattern,discipline_id,limit.clamp(1,500),offset],
        |row| Ok(ProjectFileRecord {
            id: row.get(0)?, filename: row.get(1)?, relative_path: row.get(2)?,
            extension: row.get(3)?, byte_size: row.get(4)?, modified_at: row.get(5)?,
            sha256: row.get(6)?, root_path: row.get(7)?, status: row.get(8)?,
            linked: row.get::<_,i64>(9)? == 1, discipline_id: row.get(10)?,
            discipline_code: row.get(11)?, discipline_name: row.get(12)?
        })
    ).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows { items.push(row.map_err(|e| e.to_string())?); }
    Ok((items,total))
}

pub fn set_project_file_link(connection: &Connection, project_id: i64, file_id: i64, linked: bool, discipline_id: Option<i64>) -> Result<(),String> {
    let project_exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",params![project_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    if !project_exists { return Err("Projeto não encontrado.".into()); }
    let file_exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM files WHERE id=?1)",params![file_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    if !file_exists { return Err("Documento não encontrado na biblioteca.".into()); }
    if let Some(discipline_id) = discipline_id {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_disciplines WHERE id=?1 AND project_id=?2)",
            params![discipline_id,project_id],|r|r.get(0)
        ).map_err(|e|e.to_string())?;
        if !valid { return Err("A disciplina selecionada não pertence ao projeto.".into()); }
    }
    if linked {
        connection.execute(
            "INSERT INTO project_file_links(project_id,file_id,discipline_id,source)
             VALUES (?1,?2,?3,'manual')
             ON CONFLICT(project_id,file_id) DO UPDATE SET discipline_id=excluded.discipline_id,source='manual',linked_at=CURRENT_TIMESTAMP",
            params![project_id,file_id,discipline_id]
        ).map_err(|e|e.to_string())?;
    } else {
        connection.execute("DELETE FROM project_file_links WHERE project_id=?1 AND file_id=?2",params![project_id,file_id]).map_err(|e|e.to_string())?;
    }
    connection.execute("UPDATE projects SET updated_at=CURRENT_TIMESTAMP WHERE id=?1",params![project_id]).map_err(|e|e.to_string())?;
    Ok(())
}
