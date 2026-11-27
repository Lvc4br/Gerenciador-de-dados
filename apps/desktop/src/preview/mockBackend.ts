export const isPreviewMode =
  typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);

const sampleFiles = [
  { id: 1, filename: "AB20A-EST-001.dwg", relative_path: "Estruturas/AB20A-EST-001.dwg", extension: "dwg", byte_size: 4823441, modified_at: "2026-10-12T14:32:00Z", sha256: "a81f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0011", root_path: "C:\\Projetos\\AB20A", status: "indexed" },
  { id: 2, filename: "AB20A-EST-001.pdf", relative_path: "Estruturas/AB20A-EST-001.pdf", extension: "pdf", byte_size: 1834200, modified_at: "2026-10-12T14:30:00Z", sha256: "b92f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0022", root_path: "C:\\Projetos\\AB20A", status: "indexed" },
  { id: 3, filename: "Memorial_Descritivo.docx", relative_path: "Documentos/Memorial_Descritivo.docx", extension: "docx", byte_size: 328440, modified_at: "2026-09-28T10:15:00Z", sha256: "c03f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0033", root_path: "C:\\Projetos\\AB20A", status: "indexed" },
  { id: 4, filename: "Levantamento_Topografico.xlsx", relative_path: "Levantamentos/Levantamento_Topografico.xlsx", extension: "xlsx", byte_size: 945210, modified_at: "2026-09-17T16:05:00Z", sha256: "d14f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0044", root_path: "C:\\Projetos\\AB20A", status: "indexed" },
  { id: 5, filename: "AB20A-DRE-014.dwg", relative_path: "Drenagem/AB20A-DRE-014.dwg", extension: "dwg", byte_size: 5128900, modified_at: "2026-08-03T09:20:00Z", sha256: "e25f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0055", root_path: "C:\\Projetos\\AB20A", status: "indexed" },
  { id: 6, filename: "Planilha_Custos.xlsx", relative_path: "Orcamento/Planilha_Custos.xlsx", extension: "xlsx", byte_size: 216880, modified_at: "2026-07-22T11:42:00Z", sha256: "f36f8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0066", root_path: "C:\\Projetos\\AB20A", status: "indexed" }
];

const sampleDiscrepancies = [
  { id: 1, filename: "AB20A-EST-001.dwg", branch: "Estruturas", number_in_filename: "EST-001", number_in_titleblock: "EST-010", revision: "02", extracted_title: "Planta de formas - tabuleiro", title_for_titleblock_number: "Detalhe de encontro", diagnosis: "Número divergente", artesp_number: "ART-001", internal_number: "EST-001", source_row: 2, source_csv: "divergencias_numero_carimbo.csv" },
  { id: 2, filename: "AB20A-DRE-014.dwg", branch: "Drenagem", number_in_filename: "DRE-014", number_in_titleblock: "DRE-014", revision: "01", extracted_title: "Detalhe de drenagem", title_for_titleblock_number: "Detalhe de drenagem", diagnosis: "Mesmo número e título", artesp_number: "ART-014", internal_number: "DRE-014", source_row: 3, source_csv: "divergencias_numero_carimbo.csv" },
  { id: 3, filename: "AB20A-EST-022.dwg", branch: "Estruturas", number_in_filename: "EST-022", number_in_titleblock: "EST-023", revision: "00", extracted_title: "Corte longitudinal", title_for_titleblock_number: "Seção transversal", diagnosis: "Número divergente", artesp_number: "ART-022", internal_number: "EST-022", source_row: 4, source_csv: "divergencias_numero_carimbo.csv" }
];

export async function previewInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise(resolve => setTimeout(resolve, 80));
  let result: unknown;
  switch (command) {
    case "get_dashboard_stats":
      result = { root_count: 3, file_count: 1342, duplicate_hash_groups: 8, drawing_metadata_count: 1336, discrepancy_count: sampleDiscrepancies.length };
      break;
    case "list_files": {
      const search = String(args.search ?? "").toLocaleLowerCase("pt-BR");
      const extension = String(args.extension ?? "").toLowerCase().replace(/^\./, "");
      const sortBy = String(args.sortBy ?? "path");
      const ascending = args.ascending !== false;
      const filtered = sampleFiles.filter(file =>
        (!search || file.filename.toLocaleLowerCase("pt-BR").includes(search) || file.relative_path.toLocaleLowerCase("pt-BR").includes(search)) &&
        (!extension || file.extension === extension)
      );
      filtered.sort((a, b) => String(a[sortBy as keyof typeof a] ?? a.relative_path).localeCompare(String(b[sortBy as keyof typeof b] ?? b.relative_path), "pt-BR") * (ascending ? 1 : -1));
      const offset = Number(args.offset ?? 0);
      const limit = Number(args.limit ?? 100);
      result = { items: filtered.slice(offset, offset + limit), total: filtered.length };
      break;
    }
    case "list_file_versions":
      result = [
        { id: 2, sha256: sampleFiles[0].sha256, byte_size: sampleFiles[0].byte_size, modified_at: sampleFiles[0].modified_at, captured_at: "2026-10-12T14:32:00Z" },
        { id: 1, sha256: "91af8f6d1a7b9d3e4c5f6a7b8c9d00112233445566778899aabbccddeeff0099", byte_size: 4701200, modified_at: "2026-08-01T10:00:00Z", captured_at: "2026-08-01T10:05:00Z" }
      ];
      break;
    case "list_discrepancies": {
      const search = String(args.search ?? "").toLocaleLowerCase("pt-BR");
      const filtered = sampleDiscrepancies.filter(item =>
        !search || [item.filename, item.branch, item.number_in_filename, item.number_in_titleblock, item.diagnosis].some(value => value.toLocaleLowerCase("pt-BR").includes(search))
      );
      const offset = Number(args.offset ?? 0);
      const limit = Number(args.limit ?? 100);
      result = { items: filtered.slice(offset, offset + limit), total: filtered.length };
      break;
    }
    case "scan_folder":
      result = { root: "C:\\Projetos\\Exemplo", indexed: 42, skipped: 1, errors: 0, missing: 0, reconciliation_complete: true, duplicate_hash_groups: 2 };
      break;
    case "create_database_backup":
      result = "C:\\GED\\backups\\ged-backup.sqlite3";
      break;
    case "restore_database_backup":
      result = "C:\\GED\\backups\\ged-pre-restore-demo.sqlite3";
      break;
    default:
      throw new Error(`Comando não disponível na prévia: ${command}`);
  }
  return result as T;
}
