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

let sampleProjects = [
  { id: 1, code: 'AB20A', name: 'Adequação da Ponte AB20A', client: 'ARTESP', description: 'Projeto demonstrativo de engenharia', status: 'active', root_count: 1, file_count: 6, created_at: '2026-11-01', updated_at: '2026-11-20' }
];
let nextProjectId = 2;
const sampleRoots = [{ id: 1, path: 'C:\\\\Projetos\\\\AB20A', added_at: '2026-10-01' }];
let sampleProjectRootLinks: Record<number, number[]> = { 1: [1] };\nlet sampleDisciplines: Array<{id:number;project_id:number;code:string;name:string;sort_order:number}> = [\n  { id: 1, project_id: 1, code: 'EST', name: 'Estruturas', sort_order: 0 },\n  { id: 2, project_id: 1, code: 'DRE', name: 'Drenagem', sort_order: 1 }\n];\nlet nextDisciplineId = 3;\nlet sampleProjectFileLinks: Record<string, number | null> = { '1:1': 1, '1:5': 2 };

const sampleDiscrepancies = [
  { id: 1, filename: "AB20A-EST-001.dwg", branch: "Estruturas", number_in_filename: "EST-001", number_in_titleblock: "EST-010", revision: "02", extracted_title: "Planta de formas - tabuleiro", title_for_titleblock_number: "Detalhe de encontro", diagnosis: "Número divergente", artesp_number: "ART-001", internal_number: "EST-001", source_row: 2, source_csv: "divergencias_numero_carimbo.csv" },
  { id: 2, filename: "AB20A-DRE-014.dwg", branch: "Drenagem", number_in_filename: "DRE-014", number_in_titleblock: "DRE-014", revision: "01", extracted_title: "Detalhe de drenagem", title_for_titleblock_number: "Detalhe de drenagem", diagnosis: "Mesmo número e título", artesp_number: "ART-014", internal_number: "DRE-014", source_row: 3, source_csv: "divergencias_numero_carimbo.csv" },
  { id: 3, filename: "AB20A-EST-022.dwg", branch: "Estruturas", number_in_filename: "EST-022", number_in_titleblock: "EST-023", revision: "00", extracted_title: "Corte longitudinal", title_for_titleblock_number: "Seção transversal", diagnosis: "Número divergente", artesp_number: "ART-022", internal_number: "EST-022", source_row: 4, source_csv: "divergencias_numero_carimbo.csv" }
];

export async function previewInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise(resolve => setTimeout(resolve, 80));
  let result: unknown;
  switch (command) {
    case "list_projects": {
      const search = String(args.search ?? "").toLocaleLowerCase("pt-BR");
      const status = String(args.status ?? "");
      const filtered = sampleProjects.filter(p =>
        (!search || [p.code,p.name,p.client].some(v => v.toLocaleLowerCase("pt-BR").includes(search))) &&
        (!status || status === "all" || p.status === status)
      );
      const offset = Number(args.offset ?? 0), limit = Number(args.limit ?? 100);
      result = { items: filtered.slice(offset, offset + limit), total: filtered.length };
      break;
    }
    case "create_project": {
      const code = String(args.code ?? "").trim().toUpperCase();
      const name = String(args.name ?? "").trim();
      if (!code || !name) throw new Error("Código e nome são obrigatórios.");
      if (sampleProjects.some(p => p.code.toUpperCase() === code)) throw new Error("Já existe um projeto com esse código.");
      const item = { id: nextProjectId++, code, name, client: String(args.client ?? ""), description: String(args.description ?? ""), status: "active", root_count: 0, file_count: 0, created_at: new Date().toISOString(), updated_at: new Date().toISOString() };
      sampleProjects = [...sampleProjects, item];
      result = item.id;
      break;
    }
    case "update_project": {
      const id = Number(args.id), code = String(args.code ?? "").trim().toUpperCase(), name = String(args.name ?? "").trim();
      if (sampleProjects.some(p => p.id !== id && p.code.toUpperCase() === code)) throw new Error("Já existe outro projeto com esse código.");
      sampleProjects = sampleProjects.map(p => p.id === id ? { ...p, code, name, client: String(args.client ?? ""), description: String(args.description ?? ""), updated_at: new Date().toISOString() } : p);
      result = null;
      break;
    }
    case "archive_project": {
      const id = Number(args.id), status = args.archived ? "archived" : "active";
      sampleProjects = sampleProjects.map(p => p.id === id ? { ...p, status } : p);
      result = null;
      break;
    }
    case "list_project_disciplines": {
      result = sampleDisciplines.filter(d => d.project_id === Number(args.projectId)).sort((a,b) => a.sort_order-b.sort_order);
      break;
    }
    case "save_project_discipline": {
      const projectId = Number(args.projectId), code = String(args.code ?? "").trim().toUpperCase(), name = String(args.name ?? "").trim();
      if (!code || !name) throw new Error("Código e nome da disciplina são obrigatórios.");
      if (sampleDisciplines.some(d => d.project_id === projectId && d.code.toUpperCase() === code)) throw new Error("Já existe uma disciplina com esse código neste projeto.");
      const id = nextDisciplineId++;
      sampleDisciplines = [...sampleDisciplines, { id, project_id: projectId, code, name, sort_order: Number(args.sortOrder ?? 0) }];
      result = id;
      break;
    }
    case "delete_project_discipline": {
      const projectId = Number(args.projectId), disciplineId = Number(args.disciplineId);
      sampleDisciplines = sampleDisciplines.filter(d => !(d.project_id === projectId && d.id === disciplineId));
      for (const key of Object.keys(sampleProjectFileLinks)) if (key.startsWith(projectId + ":") && sampleProjectFileLinks[key] === disciplineId) sampleProjectFileLinks[key] = null;
      result = null;
      break;
    }
    case "list_project_files": {
      const projectId = Number(args.projectId), search = String(args.search ?? "").toLocaleLowerCase("pt-BR");
      const linkedRoots = sampleProjectRootLinks[projectId] ?? [];
      const filtered = sampleFiles.filter(file =>
        (linkedRoots.includes(1) || sampleProjectFileLinks[projectId + ":" + file.id] !== undefined) &&
        (!search || file.filename.toLocaleLowerCase("pt-BR").includes(search) || file.relative_path.toLocaleLowerCase("pt-BR").includes(search))
      ).map(file => {
        const key = projectId + ":" + file.id;
        const linked = Object.prototype.hasOwnProperty.call(sampleProjectFileLinks, key);
        const disciplineId = linked ? sampleProjectFileLinks[key] : null;
        const discipline = sampleDisciplines.find(d => d.id === disciplineId && d.project_id === projectId);
        return { ...file, linked, discipline_id: disciplineId, discipline_code: discipline?.code ?? null, discipline_name: discipline?.name ?? null };
      });
      result = { items: filtered.slice(Number(args.offset ?? 0), Number(args.offset ?? 0) + Number(args.limit ?? 100)), total: filtered.length };
      break;
    }
    case "set_project_file_link": {
      const projectId = Number(args.projectId), fileId = Number(args.fileId), disciplineId = args.disciplineId == null ? null : Number(args.disciplineId);
      if (disciplineId !== null && !sampleDisciplines.some(d => d.id === disciplineId && d.project_id === projectId)) throw new Error("A disciplina selecionada não pertence ao projeto.");
      const key = projectId + ":" + fileId;
      if (args.linked) sampleProjectFileLinks[key] = disciplineId;
      else delete sampleProjectFileLinks[key];
      const linkedRootIds = sampleProjectRootLinks[projectId] ?? [];
      const linkedFileIds = Object.keys(sampleProjectFileLinks)
        .filter(item => item.startsWith(projectId + ":"))
        .map(item => Number(item.split(":")[1]));
      const fileCount = new Set([
        ...sampleFiles.filter(file => linkedRootIds.includes(1)).map(file => file.id),
        ...linkedFileIds
      ]).size;
      sampleProjects = sampleProjects.map(project => project.id === projectId ? { ...project, file_count: fileCount } : project);
      result = null;
      break;
    }
    case "list_project_roots": {
      const projectId = Number(args.projectId);
      result = sampleRoots.map(root => ({ ...root, linked: (sampleProjectRootLinks[projectId] ?? []).includes(root.id) }));
      break;
    }
    case "set_project_root_link": {
      const projectId = Number(args.projectId), rootId = Number(args.rootId);
      const links = sampleProjectRootLinks[projectId] ?? [];
      sampleProjectRootLinks[projectId] = args.linked ? [...new Set([...links, rootId])] : links.filter(id => id !== rootId);
      const rootCount = sampleProjectRootLinks[projectId].length;
      sampleProjects = sampleProjects.map(p => p.id === projectId ? { ...p, root_count: rootCount, file_count: rootCount ? 6 : 0 } : p);
      result = null;
      break;
    }
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
