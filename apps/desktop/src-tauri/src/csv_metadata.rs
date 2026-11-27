use rusqlite::{params, Connection};
use std::{collections::HashMap, fs, path::Path};

fn cell<'a>(record: &'a csv::StringRecord, headers: &HashMap<String, usize>, name: &str) -> &'a str {
    headers.get(name).and_then(|index| record.get(*index)).unwrap_or("").trim()
}

pub fn ensure_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS drawing_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            source_row INTEGER NOT NULL,
            filename TEXT NOT NULL DEFAULT '',
            branch TEXT NOT NULL DEFAULT '',
            candidate_title TEXT NOT NULL DEFAULT '',
            road TEXT NOT NULL DEFAULT '',
            segment TEXT NOT NULL DEFAULT '',
            scale TEXT NOT NULL DEFAULT '',
            artesp_number TEXT NOT NULL DEFAULT '',
            internal_number TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            raw_json TEXT NOT NULL,
            imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(source_file_id, source_row)
         );
         CREATE INDEX IF NOT EXISTS idx_drawing_metadata_filename ON drawing_metadata(filename);
         CREATE INDEX IF NOT EXISTS idx_drawing_metadata_internal ON drawing_metadata(internal_number);
         CREATE INDEX IF NOT EXISTS idx_drawing_metadata_artesp ON drawing_metadata(artesp_number);

         CREATE TABLE IF NOT EXISTS drawing_discrepancies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            source_row INTEGER NOT NULL,
            filename TEXT NOT NULL DEFAULT '',
            branch TEXT NOT NULL DEFAULT '',
            number_in_filename TEXT NOT NULL DEFAULT '',
            number_in_titleblock TEXT NOT NULL DEFAULT '',
            revision TEXT NOT NULL DEFAULT '',
            extracted_title TEXT NOT NULL DEFAULT '',
            title_for_titleblock_number TEXT NOT NULL DEFAULT '',
            diagnosis TEXT NOT NULL DEFAULT '',
            artesp_number TEXT NOT NULL DEFAULT '',
            internal_number TEXT NOT NULL DEFAULT '',
            raw_json TEXT NOT NULL,
            imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(source_file_id, source_row)
         );
         CREATE INDEX IF NOT EXISTS idx_discrepancies_filename ON drawing_discrepancies(filename);
         CREATE INDEX IF NOT EXISTS idx_discrepancies_diagnosis ON drawing_discrepancies(diagnosis);"
    ).map_err(|e| e.to_string())
}

pub fn import_metadata_csv(
    connection: &Connection,
    source_file_id: i64,
    path: &Path,
) -> Result<usize, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let first_line = bytes.split(|byte| *byte == b'\n').next().unwrap_or(&[]);
    let semicolons = first_line.iter().filter(|byte| **byte == b';').count();
    let commas = first_line.iter().filter(|byte| **byte == b',').count();
    let delimiter = if semicolons >= commas { b';' } else { b',' };

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(bytes.as_slice());
    let headers_record = reader.headers().map_err(|e| e.to_string())?.clone();
    let headers: HashMap<String, usize> = headers_record.iter().enumerate()
        .map(|(index, name)| (name.trim_start_matches('\u{feff}').trim().to_lowercase(), index))
        .collect();

    let is_title_metadata = [
        "arquivo", "ramo", "candidato_a_titulo", "rodovia", "trecho",
        "escala", "desenho_artesp", "desenho_interno"
    ].iter().all(|name| headers.contains_key(*name));

    let is_discrepancy = [
        "arquivo", "ramo", "numero_no_nome", "numero_no_carimbo",
        "revisao", "titulo_extraido", "titulo_do_numero_do_carimbo", "diagnostico"
    ].iter().all(|name| headers.contains_key(*name));

    if !is_title_metadata && !is_discrepancy {
        connection.execute("DELETE FROM drawing_metadata WHERE source_file_id=?1", params![source_file_id])
            .map_err(|e| e.to_string())?;
        connection.execute("DELETE FROM drawing_discrepancies WHERE source_file_id=?1", params![source_file_id])
            .map_err(|e| e.to_string())?;
        return Ok(0);
    }

    // A source file can change schema between imports; clear the other dataset first.
    if is_discrepancy {
        connection.execute("DELETE FROM drawing_metadata WHERE source_file_id=?1", params![source_file_id])
            .map_err(|e| e.to_string())?;
    } else {
        connection.execute("DELETE FROM drawing_discrepancies WHERE source_file_id=?1", params![source_file_id])
            .map_err(|e| e.to_string())?;
    }

    let mut imported = 0usize;
    for (index, result) in reader.records().enumerate() {
        let record = result.map_err(|e| e.to_string())?;
        let raw: HashMap<String, String> = headers.iter()
            .map(|(name, position)| (name.clone(), record.get(*position).unwrap_or("").to_string()))
            .collect();
        let raw_json = serde_json::to_string(&raw).map_err(|e| e.to_string())?;
        let source_row = (index + 2) as i64;

        if is_discrepancy {
            connection.execute(
                "INSERT INTO drawing_discrepancies(
                    source_file_id, source_row, filename, branch, number_in_filename,
                    number_in_titleblock, revision, extracted_title, title_for_titleblock_number,
                    diagnosis, artesp_number, internal_number, raw_json
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
                 ON CONFLICT(source_file_id,source_row) DO UPDATE SET
                    filename=excluded.filename, branch=excluded.branch,
                    number_in_filename=excluded.number_in_filename,
                    number_in_titleblock=excluded.number_in_titleblock,
                    revision=excluded.revision, extracted_title=excluded.extracted_title,
                    title_for_titleblock_number=excluded.title_for_titleblock_number,
                    diagnosis=excluded.diagnosis, artesp_number=excluded.artesp_number,
                    internal_number=excluded.internal_number, raw_json=excluded.raw_json,
                    imported_at=CURRENT_TIMESTAMP",
                params![
                    source_file_id, source_row, cell(&record,&headers,"arquivo"),
                    cell(&record,&headers,"ramo"), cell(&record,&headers,"numero_no_nome"),
                    cell(&record,&headers,"numero_no_carimbo"), cell(&record,&headers,"revisao"),
                    cell(&record,&headers,"titulo_extraido"),
                    cell(&record,&headers,"titulo_do_numero_do_carimbo"),
                    cell(&record,&headers,"diagnostico"), cell(&record,&headers,"desenho_artesp"),
                    cell(&record,&headers,"desenho_interno"), raw_json
                ]
            ).map_err(|e| e.to_string())?;
        } else {
            connection.execute(
                "INSERT INTO drawing_metadata(
                    source_file_id, source_row, filename, branch, candidate_title, road,
                    segment, scale, artesp_number, internal_number, note, raw_json
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(source_file_id,source_row) DO UPDATE SET
                    filename=excluded.filename, branch=excluded.branch,
                    candidate_title=excluded.candidate_title, road=excluded.road,
                    segment=excluded.segment, scale=excluded.scale,
                    artesp_number=excluded.artesp_number, internal_number=excluded.internal_number,
                    note=excluded.note, raw_json=excluded.raw_json,
                    imported_at=CURRENT_TIMESTAMP",
                params![
                    source_file_id, source_row, cell(&record,&headers,"arquivo"),
                    cell(&record,&headers,"ramo"), cell(&record,&headers,"candidato_a_titulo"),
                    cell(&record,&headers,"rodovia"), cell(&record,&headers,"trecho"),
                    cell(&record,&headers,"escala"), cell(&record,&headers,"desenho_artesp"),
                    cell(&record,&headers,"desenho_interno"), cell(&record,&headers,"nota"),
                    raw_json
                ]
            ).map_err(|e| e.to_string())?;
        }
        imported += 1;
    }

    // A successful full import is authoritative for this source file.
    // Remove rows left behind when the CSV has fewer data records than before.
    let last_source_row = (imported + 1) as i64;
    if is_discrepancy {
        connection.execute(
            "DELETE FROM drawing_discrepancies WHERE source_file_id=?1 AND source_row>?2",
            params![source_file_id, last_source_row]
        ).map_err(|e| e.to_string())?;
    } else {
        connection.execute(
            "DELETE FROM drawing_metadata WHERE source_file_id=?1 AND source_row>?2",
            params![source_file_id, last_source_row]
        ).map_err(|e| e.to_string())?;
    }
    Ok(imported)
}
