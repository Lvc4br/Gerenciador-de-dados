import { useEffect, useState, type FormEvent } from "react";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { isPreviewMode, previewInvoke } from "./preview/mockBackend";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Archive, ArrowDownToLine, Bell, BookOpen, ChevronRight,
  CircleAlert, Clock3, FileCheck2, FileSearch, Files, Folder, FolderOpen,
  HardDrive, LayoutDashboard, Search, Settings2, ShieldCheck, SlidersHorizontal,
  FileText, RefreshCw, ChevronLeft, ArrowUpDown
} from "lucide-react";

type Section = "Visão geral" | "Projetos" | "Explorador" | "Revisão" | "Divergências" | "Exportações";
type DashboardStats = { root_count: number; file_count: number; duplicate_hash_groups: number; drawing_metadata_count: number; discrepancy_count: number };
type ScanSummary = { root: string; indexed: number; skipped: number; errors: number; missing: number; reconciliation_complete: boolean; duplicate_hash_groups: number };
type ScanProgress = { root: string; visited: number; indexed: number; skipped: number; errors: number };

const navigation: { label: Section; icon: typeof LayoutDashboard }[] = [
  { label: "Visão geral", icon: LayoutDashboard },
  { label: "Projetos", icon: Folder },
  { label: "Explorador", icon: Files },
  { label: "Revisão", icon: FileCheck2 },
  { label: "Divergências", icon: CircleAlert },
  { label: "Exportações", icon: ArrowDownToLine }
];

const emptyStats: DashboardStats = { root_count: 0, file_count: 0, duplicate_hash_groups: 0, drawing_metadata_count: 0, discrepancy_count: 0 };


type FileRecord = {
  id: number; filename: string; relative_path: string; extension: string;
  byte_size: number; modified_at: string; sha256: string; root_path: string; status: string;
};
type FilePage = { items: FileRecord[]; total: number };
type FileVersion = { id: number; sha256: string; byte_size: number; modified_at: string; captured_at: string };

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isPreviewMode) return previewInvoke<T>(command, args);
  return tauriInvoke<T>(command, args);
}

function formatDate(value: string, includeTime = false) {
  if (!value) return '—';
  const numeric = Number(value);
  const date = new Date(Number.isFinite(numeric) ? numeric * 1000 : value);
  return Number.isNaN(date.getTime()) ? '—' : (includeTime ? date.toLocaleString('pt-BR') : date.toLocaleDateString('pt-BR'));
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return String(bytes) + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}

function ExplorerPanel() {
  const [search, setSearch] = useState('');
  const [extension, setExtension] = useState('');
  const [clientFilter, setClientFilter] = useState('');
  const [projectFilter, setProjectFilter] = useState('');
  const [disciplineFilter, setDisciplineFilter] = useState('');
  const [disciplineOptions, setDisciplineOptions] = useState<Array<{id:number;code:string;name:string}>>([]);
  const [sortBy, setSortBy] = useState('path');
  const [ascending, setAscending] = useState(true);
  const [page, setPage] = useState(0);
  const [data, setData] = useState<FilePage>({ items: [], total: 0 });
  const [selected, setSelected] = useState<FileRecord | null>(null);
  const [versions, setVersions] = useState<FileVersion[]>([]);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [projectOptions, setProjectOptions] = useState<Array<{id:number;code:string;name:string}>>([]);
  const [targetProjectId, setTargetProjectId] = useState('');
  const [linkNotice, setLinkNotice] = useState('');
  const [linking, setLinking] = useState(false);
  const pageSize = 100;

  useEffect(() => {
    invoke<{items:Array<{id:number;code:string;name:string}>}>('list_projects', { search: '', status: 'active', limit: 500, offset: 0 })
      .then(result => {
        setProjectOptions(result.items);
        setTargetProjectId(current => current || (result.items[0] ? String(result.items[0].id) : ''));
      })
      .catch(() => setProjectOptions([]));
  }, []);

  useEffect(() => {
    setDisciplineFilter('');
    if (!projectFilter) { setDisciplineOptions([]); return; }
    invoke<Array<{id:number;code:string;name:string}>>('list_project_disciplines', { projectId: Number(projectFilter) })
      .then(setDisciplineOptions)
      .catch(() => setDisciplineOptions([]));
  }, [projectFilter]);

  async function linkSelectedFile() {
    if (!selected || !targetProjectId) return;
    setLinking(true); setLinkNotice(''); setError('');
    try {
      await invoke<void>('set_project_file_link', {
        projectId: Number(targetProjectId), fileId: selected.id, linked: true, disciplineId: null
      });
      const project = projectOptions.find(item => item.id === Number(targetProjectId));
      setLinkNotice(project ? selected.filename + ' foi associado ao projeto ' + project.code + '.' : 'Documento associado ao projeto.');
    } catch (reason) { setError(String(reason)); }
    finally { setLinking(false); }
  }

  async function loadFiles() {
    setLoading(true); setError('');
    try {
      const result = await invoke<FilePage>('list_files', {
        search, extension, client: clientFilter,
        projectId: projectFilter ? Number(projectFilter) : null,
        disciplineId: disciplineFilter ? Number(disciplineFilter) : null,
        sortBy, ascending, limit: pageSize, offset: page * pageSize
      });
      setData(result);
      setSelected(current => current
        ? (result.items.find(item => item.id === current.id) ?? result.items[0] ?? null)
        : (result.items[0] ?? null));
    } catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }

  useEffect(() => { void loadFiles(); }, [search, extension, clientFilter, projectFilter, disciplineFilter, sortBy, ascending, page]);

  useEffect(() => {
    let active = true;
    if (!selected) { setVersions([]); setLoadingVersions(false); return; }
    setLoadingVersions(true);
    invoke<FileVersion[]>('list_file_versions', { fileId: selected.id })
      .then(result => { if (active) setVersions(result); })
      .catch(() => { if (active) setVersions([]); })
      .finally(() => { if (active) setLoadingVersions(false); });
    return () => { active = false; };
  }, [selected?.id, selected?.sha256]);

  const totalPages = Math.max(1, Math.ceil(data.total / pageSize));
  return (
    <section className="explorer">
      <div className="page-heading explorer-heading">
        <div><div className="eyebrow">BIBLIOTECA LOCAL</div><h1>Explorador de arquivos</h1><p>Pesquise e inspecione os arquivos registrados nas pastas indexadas.</p></div>
        <button className="secondary-button" onClick={() => void loadFiles()} disabled={loading}><RefreshCw size={15} /> Atualizar</button>
      </div>
      <div className="explorer-toolbar">
        <label className="explorer-search"><Search size={16} /><input value={search} onChange={e => { setSearch(e.target.value); setPage(0); }} placeholder="Nome do arquivo ou caminho..." /></label>
        <label className="filter-control"><span>Cliente</span><input value={clientFilter} onChange={e => { setClientFilter(e.target.value); setPage(0); }} placeholder="Nome do cliente" /></label>
        <label className="filter-control"><span>Projeto</span><select value={projectFilter} onChange={e => { setProjectFilter(e.target.value); setPage(0); }}><option value="">Todos</option>{projectOptions.map(project => <option key={project.id} value={project.id}>{project.code} — {project.name}</option>)}</select></label>
        <label className="filter-control"><span>Disciplina</span><select value={disciplineFilter} onChange={e => { setDisciplineFilter(e.target.value); setPage(0); }} disabled={!projectFilter}><option value="">Todas</option>{disciplineOptions.map(d => <option key={d.id} value={d.id}>{d.code} — {d.name}</option>)}</select></label>
        <label className="filter-control"><span>Extensão</span><input value={extension} onChange={e => { setExtension(e.target.value); setPage(0); }} placeholder="ex.: pdf, dwg" /></label>
        <label className="filter-control"><span>Ordenar por</span><select value={sortBy} onChange={e => setSortBy(e.target.value)}><option value="path">Caminho</option><option value="filename">Nome</option><option value="extension">Extensão</option><option value="size">Tamanho</option><option value="modified">Modificação</option></select></label>
        <button className="sort-button" title="Inverter ordenação" onClick={() => setAscending(value => !value)}><ArrowUpDown size={16} /> {ascending ? 'Crescente' : 'Decrescente'}</button>
      </div>
      <div className="explorer-summary"><span>{loading ? 'Consultando biblioteca...' : data.total.toLocaleString('pt-BR') + ' arquivo(s) encontrado(s)'}</span><span>Exibindo até 100 por página</span></div>
      {error && <div className="notice error">{error}</div>}
      <div className="explorer-layout">
        <div className="file-table-wrap">
          <table className="file-table">
            <thead><tr><th>Nome</th><th>Tipo</th><th>Tamanho</th><th>Última modificação</th><th>Status</th></tr></thead>
            <tbody>
              {data.items.map(file => <tr key={file.id} className={selected?.id === file.id ? 'selected' : ''} onClick={() => setSelected(file)}>
                <td><div className="file-name"><FileText size={16} /><div><b>{file.filename}</b><small>{file.relative_path}</small></div></div></td>
                <td><span className="file-ext">{file.extension ? file.extension.toUpperCase() : 'SEM TIPO'}</span></td>
                <td>{formatBytes(file.byte_size)}</td>
                <td>{formatDate(file.modified_at)}</td>
                <td><span className={`file-status ${file.status}`}>{file.status === 'missing' ? 'Ausente' : file.status === 'skipped' ? 'Ignorado' : file.status === 'error' ? 'Erro' : 'Indexado'}</span></td>
              </tr>)}
              {!loading && data.items.length === 0 && <tr><td colSpan={5} className="table-empty">Nenhum arquivo encontrado. Importe uma pasta na Visão geral para começar.</td></tr>}
            </tbody>
          </table>
          <div className="pagination"><span>Página {page + 1} de {totalPages}</span><div><button disabled={page === 0} onClick={() => setPage(value => Math.max(0, value - 1))}><ChevronLeft size={15} /> Anterior</button><button disabled={page + 1 >= totalPages} onClick={() => setPage(value => Math.min(totalPages - 1, value + 1))}>Próxima <ChevronRight size={15} /></button></div></div>
        </div>
        <aside className="file-details">
          <div className="details-heading"><FileText size={18} /><b>Propriedades</b></div>
          {selected ? <>
            <h3>{selected.filename}</h3>
            <div className="detail-row"><span>Localização</span><b>{selected.root_path}\\{selected.relative_path}</b></div>
            <div className="detail-row"><span>Extensão</span><b>{selected.extension || '—'}</b></div>
            <div className="detail-row"><span>Tamanho</span><b>{formatBytes(selected.byte_size)}</b></div>
            <div className="detail-row"><span>Modificado</span><b>{formatDate(selected.modified_at, true)}</b></div>
            <div className="detail-row hash-row"><span>SHA-256 atual</span><code>{selected.sha256}</code></div>
            <div className="project-link-box">
              <b>Adicionar a um projeto</b>
              <select value={targetProjectId} onChange={event => setTargetProjectId(event.target.value)}>
                <option value="">Selecione um projeto...</option>
                {projectOptions.map(project => <option key={project.id} value={project.id}>{project.code} — {project.name}</option>)}
              </select>
              <button className="secondary-button" disabled={!targetProjectId || linking || selected.status !== 'indexed'} onClick={() => void linkSelectedFile()}>
                {linking ? 'Associando...' : 'Vincular documento'}
              </button>
              {linkNotice && <small role="status">{linkNotice}</small>}
              {projectOptions.length === 0 && <small>Cadastre um projeto antes de associar documentos.</small>}
            </div>
            <div className="version-history">
              <div className="details-heading"><Clock3 size={16} /><b>Histórico de versões</b><span>{loadingVersions ? 'Carregando...' : versions.length}</span></div>
              {versions.length === 0 && !loadingVersions && <p className="detail-note">Nenhuma versão registrada. Execute uma nova indexação para registrar o conteúdo atual.</p>}
              {versions.map((version, index) => <div className="version-item" key={version.id}>
                <div><b>{index === 0 ? 'Versão mais recente' : `Versão ${versions.length - index}`}</b><small>{formatDate(version.captured_at, true)}</small></div>
                <span>{formatBytes(version.byte_size)}</span>
                <code>{version.sha256}</code>
              </div>)}
            </div>
            <p className="detail-note">O GED registra uma entrada por conteúdo SHA-256 distinto. O arquivo original permanece no local de origem; esta função ainda não armazena uma cópia física de cada versão.</p>
          </> : <p className="detail-note">Selecione um arquivo na lista para consultar suas propriedades.</p>}
        </aside>
      </div>
    </section>
  );
}


type DiscrepancyRecord = {
  id: number; filename: string; branch: string; number_in_filename: string;
  number_in_titleblock: string; revision: string; extracted_title: string;
  title_for_titleblock_number: string; diagnosis: string; artesp_number: string;
  internal_number: string; source_row: number; source_csv: string;
};
type DiscrepancyPage = { items: DiscrepancyRecord[]; total: number };

function DiscrepanciesPanel() {
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(0);
  const [data, setData] = useState<DiscrepancyPage>({ items: [], total: 0 });
  const [selected, setSelected] = useState<DiscrepancyRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const pageSize = 100;

  async function loadDiscrepancies() {
    setLoading(true); setError('');
    try {
      const result = await invoke<DiscrepancyPage>('list_discrepancies', {
        search, limit: pageSize, offset: page * pageSize
      });
      setData(result);
      setSelected(current => current && result.items.some(item => item.id === current.id)
        ? current : (result.items[0] ?? null));
    } catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }

  useEffect(() => { void loadDiscrepancies(); }, [search, page]);
  const totalPages = Math.max(1, Math.ceil(data.total / pageSize));

  return (
    <section className="explorer">
      <div className="page-heading explorer-heading">
        <div><div className="eyebrow">CONTROLE DE QUALIDADE</div><h1>Divergências identificadas</h1><p>Ocorrências importadas dos relatórios de análise, com os dados de origem preservados.</p></div>
      </div>
      <div className="explorer-toolbar">
        <label className="explorer-search"><Search size={16} /><input value={search} onChange={e => { setSearch(e.target.value); setPage(0); }} placeholder="Buscar arquivo, ramo, número ou diagnóstico..." /></label>
      </div>
      <div className="explorer-summary"><span>{loading ? 'Consultando ocorrências...' : data.total.toLocaleString('pt-BR') + ' divergência(s) registrada(s)'}</span><span>Os diagnósticos são preservados como vieram da fonte</span></div>
      {error && <div className="notice error">{error}</div>}
      <div className="explorer-layout discrepancy-layout">
        <div className="file-table-wrap">
          <table className="file-table discrepancy-table">
            <thead><tr><th>Arquivo</th><th>Nº no nome</th><th>Nº no carimbo</th><th>Diagnóstico</th></tr></thead>
            <tbody>
              {data.items.map(item => <tr key={item.id} className={selected?.id === item.id ? 'selected' : ''} onClick={() => setSelected(item)}>
                <td><div className="file-name"><CircleAlert size={16} /><div><b>{item.filename}</b><small>{item.branch}</small></div></div></td>
                <td>{item.number_in_filename}</td>
                <td>{item.number_in_titleblock}</td>
                <td><span className={item.diagnosis.toLowerCase().startsWith('mesmo') ? 'finding-tag copy' : 'finding-tag mismatch'}>{item.diagnosis}</span></td>
              </tr>)}
              {!loading && data.items.length === 0 && <tr><td colSpan={4} className="table-empty">Nenhuma divergência importada. Importe uma pasta que contenha um CSV com o esquema de divergências reconhecido.</td></tr>}
            </tbody>
          </table>
          <div className="pagination"><span>Página {page + 1} de {totalPages}</span><div><button disabled={page === 0} onClick={() => setPage(value => Math.max(0, value - 1))}><ChevronLeft size={15} /> Anterior</button><button disabled={page + 1 >= totalPages} onClick={() => setPage(value => Math.min(totalPages - 1, value + 1))}>Próxima <ChevronRight size={15} /></button></div></div>
        </div>
        <aside className="file-details">
          <div className="details-heading"><CircleAlert size={18} /><b>Detalhes da ocorrência</b></div>
          {selected ? <>
            <h3>{selected.filename}</h3>
            <div className="detail-row"><span>Ramo</span><b>{selected.branch}</b></div>
            <div className="detail-row"><span>Número no nome</span><b>{selected.number_in_filename}</b></div>
            <div className="detail-row"><span>Número no carimbo</span><b>{selected.number_in_titleblock}</b></div>
            <div className="detail-row"><span>Revisão</span><b>{selected.revision || '—'}</b></div>
            <div className="detail-row"><span>Título extraído</span><b>{selected.extracted_title}</b></div>
            <div className="detail-row"><span>Título associado ao número do carimbo</span><b>{selected.title_for_titleblock_number}</b></div>
            <div className="detail-row"><span>Diagnóstico da fonte</span><b>{selected.diagnosis}</b></div>
            <div className="detail-row"><span>Desenho ARTESP</span><b>{selected.artesp_number}</b></div>
            <div className="detail-row"><span>Desenho interno</span><b>{selected.internal_number}</b></div>
            <p className="detail-note">Origem: {selected.source_csv}, linha {selected.source_row}. O GED exibe o diagnóstico importado; a confirmação da divergência exige revisão do responsável técnico.</p>
          </> : <p className="detail-note">Selecione uma ocorrência para consultar os campos de origem.</p>}
        </aside>
      </div>
    </section>
  );
}


type ProjectRecord = {
  id: number; code: string; name: string; client: string; description: string;
  status: string; root_count: number; file_count: number; created_at: string; updated_at: string;
};
type ProjectRoot = { id: number; path: string; added_at: string; linked: boolean };
type ProjectDiscipline = { id: number; project_id: number; code: string; name: string; sort_order: number };
type ProjectFile = FileRecord & { linked: boolean; discipline_id: number | null; discipline_code: string | null; discipline_name: string | null };

function ProjectsPanel() {
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("active");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState(false);
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [client, setClient] = useState("");
  const [description, setDescription] = useState("");
  const [selected, setSelected] = useState<ProjectRecord | null>(null);
  const [roots, setRoots] = useState<ProjectRoot[]>([]);
  const [disciplines, setDisciplines] = useState<ProjectDiscipline[]>([]);
  const [projectFiles, setProjectFiles] = useState<ProjectFile[]>([]);
  const [disciplineCode, setDisciplineCode] = useState("");
  const [disciplineName, setDisciplineName] = useState("");
  const [fileSearch, setFileSearch] = useState("");

  async function loadProjects() {
    setLoading(true); setError("");
    try {
      const result = await invoke<{items: ProjectRecord[]; total: number}>("list_projects", { search, status, limit: 100, offset: 0 });
      setProjects(result.items);
      setSelected(current => current ? result.items.find(p => p.id === current.id) ?? null : null);
    } catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }
  useEffect(() => { void loadProjects(); }, [search, status]);

  async function loadProjectDetails(project: ProjectRecord, term = fileSearch) {
    setSelected(project); setError("");
    try {
      const [rootItems, disciplineItems, filePage] = await Promise.all([
        invoke<ProjectRoot[]>("list_project_roots", { projectId: project.id }),
        invoke<ProjectDiscipline[]>("list_project_disciplines", { projectId: project.id }),
        invoke<{items: ProjectFile[]; total: number}>("list_project_files", { projectId: project.id, search: term, limit: 100, offset: 0 })
      ]);
      setRoots(rootItems); setDisciplines(disciplineItems); setProjectFiles(filePage.items);
    } catch (reason) { setRoots([]); setDisciplines([]); setProjectFiles([]); setError(String(reason)); }
  }
  useEffect(() => { if (selected) void loadProjectDetails(selected, fileSearch); }, [fileSearch]);

  function resetProjectForm() {
    setCreating(false); setEditing(false); setCode(""); setName(""); setClient(""); setDescription("");
  }
  function beginEdit(project: ProjectRecord) {
    setCreating(false); setEditing(true);
    setCode(project.code); setName(project.name); setClient(project.client); setDescription(project.description);
  }
  async function saveProject(event: FormEvent) {
    event.preventDefault(); setError(""); setNotice("");
    try {
      if (editing && selected) {
        await invoke<void>("update_project", { id: selected.id, code, name, client, description });
        setNotice("Projeto atualizado.");
      } else {
        await invoke<number>("create_project", { code, name, client, description });
        setNotice("Projeto cadastrado no banco local.");
      }
      resetProjectForm();
      await loadProjects();
    } catch (reason) { setError(String(reason)); }
  }
  async function toggleArchive(project: ProjectRecord) {
    const archived = project.status !== "archived";
    try {
      await invoke<void>("archive_project", { id: project.id, archived });
      setNotice(archived ? "Projeto arquivado. Os documentos foram preservados." : "Projeto reativado.");
      await loadProjects();
    } catch (reason) { setError(String(reason)); }
  }
  async function toggleRoot(root: ProjectRoot) {
    if (!selected) return;
    try {
      await invoke<void>("set_project_root_link", { projectId: selected.id, rootId: root.id, linked: !root.linked });
      await loadProjectDetails(selected);
      await loadProjects();
    } catch (reason) { setError(String(reason)); }
  }
  async function saveDiscipline(event: FormEvent) {
    event.preventDefault();
    if (!selected) return;
    try {
      await invoke<number>("save_project_discipline", { projectId: selected.id, code: disciplineCode, name: disciplineName, sortOrder: disciplines.length });
      setDisciplineCode(""); setDisciplineName("");
      await loadProjectDetails(selected);
      setNotice("Disciplina cadastrada.");
    } catch (reason) { setError(String(reason)); }
  }
  async function deleteDiscipline(discipline: ProjectDiscipline) {
    if (!selected || !window.confirm("Excluir a disciplina " + discipline.code + "? Os documentos serão mantidos sem disciplina.")) return;
    try {
      await invoke<void>("delete_project_discipline", { projectId: selected.id, disciplineId: discipline.id });
      await loadProjectDetails(selected);
      setNotice("Disciplina excluída; os documentos foram preservados.");
    } catch (reason) { setError(String(reason)); }
  }
  async function assignFileDiscipline(file: ProjectFile, value: string) {
    if (!selected) return;
    try {
      await invoke<void>("set_project_file_link", { projectId: selected.id, fileId: file.id, linked: true, disciplineId: value ? Number(value) : null });
      await loadProjectDetails(selected);
    } catch (reason) { setError(String(reason)); }
  }

  return (
    <section className="explorer">
      <div className="page-heading explorer-heading">
        <div><div className="eyebrow">CATÁLOGO LOCAL</div><h1>Projetos</h1><p>Biblioteca central organizada por cliente, projeto e disciplina.</p></div>
        <button className="primary-button" onClick={() => { resetProjectForm(); setCreating(true); }}><Folder size={16} /> Novo projeto</button>
      </div>
      {notice && <div className="notice" role="status">{notice}</div>}
      {error && <div className="notice error" role="alert">{error}</div>}
      {(creating || editing) && <form className="surface-card project-form" onSubmit={saveProject}>
        <div className="card-heading"><div><h3>{editing ? "Editar projeto" : "Novo projeto"}</h3><p>Os dados serão salvos no SQLite local.</p></div></div>
        <div className="project-form-grid">
          <label>Código *<input required maxLength={40} value={code} onChange={e => setCode(e.target.value)} placeholder="Ex.: AB20A" /></label>
          <label>Nome do projeto *<input required maxLength={180} value={name} onChange={e => setName(e.target.value)} placeholder="Nome do empreendimento" /></label>
          <label>Cliente<input value={client} onChange={e => setClient(e.target.value)} placeholder="Cliente / contratante" /></label>
          <label>Descrição<input value={description} onChange={e => setDescription(e.target.value)} placeholder="Descrição breve (opcional)" /></label>
        </div>
        <div className="dashboard-actions"><button className="primary-button" type="submit">{editing ? "Salvar alterações" : "Salvar projeto"}</button><button className="secondary-button" type="button" onClick={resetProjectForm}>Cancelar</button></div>
      </form>}
      <div className="explorer-toolbar">
        <label className="explorer-search"><Search size={16} /><input value={search} onChange={e => setSearch(e.target.value)} placeholder="Buscar por código, nome ou cliente..." /></label>
        <label className="filter-control"><span>Situação</span><select value={status} onChange={e => setStatus(e.target.value)}><option value="active">Ativos</option><option value="archived">Arquivados</option><option value="all">Todos</option></select></label>
      </div>
      <div className="explorer-layout project-layout">
        <div className="file-table-wrap">
          <div className="explorer-summary"><span>{loading ? "Carregando projetos..." : projects.length + " projeto(s)"}</span><span>Banco SQLite local</span></div>
          <table className="file-table"><thead><tr><th>Código / Projeto</th><th>Cliente</th><th>Pastas</th><th>Documentos</th><th>Situação</th></tr></thead>
            <tbody>{projects.map(project => <tr key={project.id} className={selected?.id === project.id ? "selected" : ""} onClick={() => void loadProjectDetails(project)}>
              <td><div className="file-name"><Folder size={16} /><div><b>{project.code}</b><small>{project.name}</small></div></div></td>
              <td>{project.client || "—"}</td><td>{project.root_count}</td><td>{project.file_count.toLocaleString("pt-BR")}</td>
              <td><span className={project.status === "active" ? "file-status indexed" : "file-status missing"}>{project.status === "active" ? "Ativo" : "Arquivado"}</span></td>
            </tr>)}
            {!loading && projects.length === 0 && <tr><td colSpan={5} className="table-empty">Nenhum projeto encontrado. Cadastre o primeiro projeto para começar.</td></tr>}</tbody>
          </table>
        </div>
        <aside className="file-details">
          <div className="details-heading"><Folder size={18} /><b>Detalhes do projeto</b></div>
          {selected ? <>
            <h3>{selected.code} — {selected.name}</h3>
            <div className="detail-row"><span>Cliente</span><b>{selected.client || "—"}</b></div>
            <div className="detail-row"><span>Descrição</span><b>{selected.description || "—"}</b></div>
            <div className="detail-row"><span>Documentos</span><b>{selected.file_count}</b></div>
            <div className="dashboard-actions"><button className="secondary-button" onClick={() => beginEdit(selected)}>Editar projeto</button><button className="secondary-button" onClick={() => void toggleArchive(selected)}>{selected.status === "active" ? "Arquivar" : "Reativar"}</button></div>
            <div className="details-heading"><b>Disciplinas</b></div>
            {disciplines.map(d => <div className="detail-row" key={d.id}><b>{d.code} — {d.name}</b><button className="sort-button" onClick={() => void deleteDiscipline(d)}>Excluir</button></div>)}
            <form className="project-form-grid" onSubmit={saveDiscipline}>
              <label>Código<input required maxLength={30} value={disciplineCode} onChange={e => setDisciplineCode(e.target.value)} placeholder="EST" /></label>
              <label>Nome<input required maxLength={100} value={disciplineName} onChange={e => setDisciplineName(e.target.value)} placeholder="Estruturas" /></label>
              <button className="secondary-button" type="submit">Adicionar disciplina</button>
            </form>
            <div className="details-heading"><b>Pastas indexadas disponíveis</b></div>
            {roots.length === 0 ? <p className="detail-note">Nenhuma pasta indexada. Importe uma pasta pela Visão geral primeiro.</p> : roots.map(root => <label className="project-root-option" key={root.id}><input type="checkbox" checked={root.linked} onChange={() => void toggleRoot(root)} /><span>{root.path}</span></label>)}
            <div className="details-heading"><b>Documentos do projeto</b></div>
            <label className="explorer-search"><Search size={14} /><input value={fileSearch} onChange={e => setFileSearch(e.target.value)} placeholder="Pesquisar documentos..." /></label>
            <div className="project-document-list">
              {projectFiles.map(file => <div className="detail-row" key={file.id}>
                <div><b>{file.filename}</b><small>{file.relative_path}</small></div>
                <select aria-label={"Disciplina de " + file.filename} value={file.discipline_id ?? ""} onChange={e => void assignFileDiscipline(file,e.target.value)}>
                  <option value="">Sem disciplina</option>
                  {disciplines.map(d => <option key={d.id} value={d.id}>{d.code} — {d.name}</option>)}
                </select>
              </div>)}
              {projectFiles.length === 0 && <p className="detail-note">Nenhum documento encontrado nas pastas vinculadas.</p>}
            </div>
            <p className="detail-note">Os documentos são referências aos arquivos originais. O GED não os move nem modifica.</p>
          </> : <p className="detail-note">Selecione um projeto para consultar documentos, gerenciar disciplinas e vincular pastas indexadas.</p>}
        </aside>
      </div>
    </section>
  );
}


type FilenameSuggestion = {
  id: number; file_id: number; filename: string; relative_path: string;
  project_prefix: string | null; discipline_code: string | null;
  drawing_number: string | null; revision: string | null;
  confidence: number; status: "pending" | "accepted" | "rejected"; created_at: string;
};
type SuggestionPage = { items: FilenameSuggestion[]; total: number };

function ReviewPanel() {
  const [status, setStatus] = useState("pending");
  const [search, setSearch] = useState("");
  const [data, setData] = useState<SuggestionPage>({ items: [], total: 0 });
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  async function loadSuggestions() {
    setLoading(true); setError("");
    try {
      const result = await invoke<SuggestionPage>("list_filename_suggestions", {
        status, search, limit: 100, offset: 0
      });
      setData(result);
    } catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }

  useEffect(() => { void loadSuggestions(); }, [status, search]);

  async function review(item: FilenameSuggestion, decision: "accepted" | "rejected") {
    setBusyId(item.id); setError(""); setNotice("");
    try {
      await invoke<void>("review_filename_suggestion", { id: item.id, decision });
      setNotice(decision === "accepted"
        ? "Sugestão aceita e decisão registrada. Nenhum arquivo original foi alterado."
        : "Sugestão rejeitada e decisão registrada.");
      await loadSuggestions();
    } catch (reason) { setError(String(reason)); }
    finally { setBusyId(null); }
  }

  return (
    <section className="explorer">
      <div className="page-heading explorer-heading">
        <div><div className="eyebrow">IDENTIFICAÇÃO ASSISTIDA</div><h1>Revisão de documentos</h1><p>Confira as informações sugeridas pelo nome do arquivo antes de utilizá-las no cadastro do projeto.</p></div>
        <button className="secondary-button" onClick={() => void loadSuggestions()} disabled={loading}><RefreshCw size={15} /> Atualizar</button>
      </div>
      <div className="explorer-toolbar">
        <label className="explorer-search"><Search size={16} /><input value={search} onChange={e => setSearch(e.target.value)} placeholder="Buscar arquivo ou número do desenho..." /></label>
        <label className="filter-control"><span>Situação</span><select value={status} onChange={e => setStatus(e.target.value)}><option value="pending">Aguardando revisão</option><option value="accepted">Aceitas</option><option value="rejected">Rejeitadas</option><option value="all">Todas</option></select></label>
      </div>
      <div className="explorer-summary"><span>{loading ? "Consultando sugestões..." : data.total.toLocaleString("pt-BR") + " sugestão(ões)"}</span><span>Confiança é indicativa, não uma validação técnica</span></div>
      {(error || notice) && <div className={error ? "notice error" : "notice"} role="status">{error || notice}</div>}
      <div className="surface-card">
        {data.items.map(item => (
          <article className="review-item" key={item.id}>
            <div className="review-item-heading"><div><b>{item.filename}</b><small>{item.relative_path}</small></div><span className={"file-status " + (item.status === "pending" ? "skipped" : item.status === "accepted" ? "indexed" : "missing")}>{item.status === "pending" ? "Pendente" : item.status === "accepted" ? "Aceita" : "Rejeitada"}</span></div>
            <div className="review-metadata">
              <div><small>Projeto provável</small><b>{item.project_prefix || "—"}</b></div>
              <div><small>Disciplina</small><b>{item.discipline_code || "—"}</b></div>
              <div><small>Número do desenho</small><b>{item.drawing_number || "—"}</b></div>
              <div><small>Revisão</small><b>{item.revision || "—"}</b></div>
              <div><small>Confiança</small><b>{Math.round(item.confidence * 100)}%</b></div>
            </div>
            {item.status === "pending" && <div className="dashboard-actions review-actions">
              <button className="secondary-button" disabled={busyId === item.id} onClick={() => void review(item, "rejected")}>Rejeitar</button>
              <button className="primary-button" disabled={busyId === item.id} onClick={() => void review(item, "accepted")}>{busyId === item.id ? "Salvando..." : "Aceitar sugestão"}</button>
            </div>}
            {item.status === "accepted" && <div className="dashboard-actions review-actions">
              <button className="primary-button" disabled={busyId === item.id} onClick={async () => {
                setBusyId(item.id); setError(""); setNotice("");
                try {
                  await invoke<void>("apply_accepted_filename_suggestion", { id: item.id });
                  setNotice("Metadados confirmados e histórico de auditoria registrado. O arquivo original não foi alterado.");
                } catch (reason) { setError(String(reason)); }
                finally { setBusyId(null); }
              }}>{busyId === item.id ? "Aplicando..." : "Aplicar aos metadados confirmados"}</button>
            </div>}
          </article>
        ))}
        {!loading && data.items.length === 0 && <div className="table-empty">Nenhuma sugestão encontrada nesta situação. Execute uma indexação de pasta para analisar os nomes dos arquivos.</div>}
      </div>
      <p className="detail-note">Aceitar registra a decisão. Aplicar grava metadados confirmados e histórico de auditoria; o vínculo do documento a um projeto continua sendo uma ação separada.</p>
    </section>
  );
}

function App() {
  const [section, setSection] = useState<Section>("Visão geral");
  const [query, setQuery] = useState("");
  const [stats, setStats] = useState<DashboardStats>(emptyStats);
  const [scanning, setScanning] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  async function refreshStats() {
    try { setStats(await invoke<DashboardStats>("get_dashboard_stats")); }
    catch { /* A tela também pode ser aberta antes da primeira inicialização do banco. */ }
  }

  useEffect(() => { void refreshStats(); }, []);

  async function createBackup() {
    setError("");
    if (isPreviewMode) {
      setMessage("Prévia visual: o backup real só funciona no aplicativo desktop conectado ao SQLite.");
      return;
    }
    const destination = await save({
      title: "Salvar backup do GED",
      defaultPath: "ged-backup.sqlite3",
      filters: [{ name: "Banco SQLite GED", extensions: ["sqlite3", "db"] }]
    });
    if (typeof destination !== "string") return;
    try {
      const savedPath = await invoke<string>("create_database_backup", { destinationPath: destination });
      setMessage(`Backup criado e verificado com sucesso: ${savedPath}`);
    } catch (reason) {
      setError(String(reason));
      setMessage("");
    }
  }

  async function restoreBackup() {
    setError("");
    if (isPreviewMode) {
      setMessage("Prévia visual: a restauração real só funciona no aplicativo desktop conectado ao SQLite.");
      return;
    }
    const selected = await open({
      title: "Selecionar backup para restauração",
      directory: false,
      multiple: false,
      filters: [{ name: "Backup do GED", extensions: ["sqlite3", "db"] }]
    });
    if (typeof selected !== "string") return;
    if (!window.confirm("A restauração substituirá os dados atuais do GED. Uma cópia de segurança automática do banco atual será criada antes. Deseja continuar?")) return;
    try {
      const safetyPath = await invoke<string>("restore_database_backup", { sourcePath: selected });
      await refreshStats();
      setMessage(`Restauração concluída. O banco anterior foi preservado em: ${safetyPath}`);
    } catch (reason) {
      setError(String(reason));
      setMessage("");
    }
  }

  async function importFolder() {
    setError("");
    if (isPreviewMode) {
      setMessage("Prévia visual: para indexar uma pasta real, execute o GED em modo desktop com npm run tauri dev.");
      return;
    }
    const selected = await open({ directory: true, multiple: false, title: "Selecionar pasta de projetos" });
    if (typeof selected !== "string") return;
    setScanning(true);
    setMessage("Preparando indexação. Os originais não serão modificados.");
    let stopListening: (() => void) | undefined;
    try {
      stopListening = await listen<ScanProgress>("scan-progress", event => {
        const progress = event.payload;
        setMessage(`Verificados ${progress.visited.toLocaleString("pt-BR")} arquivos — ${progress.indexed.toLocaleString("pt-BR")} indexados, ${progress.skipped} ignorados e ${progress.errors} erros.`);
      });
      const result = await invoke<ScanSummary>("scan_folder", { folder: selected });
      await refreshStats();
      setMessage(`Indexação concluída: ${result.indexed} arquivo(s) indexado(s), ${result.skipped} ignorado(s), ${result.errors} erro(s), ${result.missing} ausente(s). ${result.duplicate_hash_groups} grupo(s) de hashes duplicados. ${result.reconciliation_complete ? "Reconciliação concluída." : "Reconciliação parcial: houve erros ao percorrer a pasta; arquivos não foram marcados como ausentes."}`);
    } catch (reason) {
      setError(String(reason));
      setMessage("");
    } finally {
      stopListening?.();
      setScanning(false);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">G</div>
          <div><strong>GED</strong><span>ENGENHARIA & DOCUMENTOS</span></div>
        </div>
        <div className="workspace-label">ÁREA DE TRABALHO</div>
        <nav>
          {navigation.map(({ label, icon: Icon }) => (
            <button key={label} className={section === label ? "nav-item active" : "nav-item"} onClick={() => setSection(label)}>
              <Icon size={18} strokeWidth={1.8} /><span>{label}</span>
              {label === "Divergências" && stats.discrepancy_count > 0 && <i className="nav-dot" />}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-status"><span className="status-led" /><div><b>Banco local</b><small>Modo offline</small></div></div>
          <button className="nav-item"><Settings2 size={18} /><span>Configurações</span></button>
          <div className="version">GED Desktop · MVP 0.1.0</div>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="breadcrumbs"><span>GED</span><ChevronRight size={14} /><b>{section}</b></div>
          <div className="top-actions">
            <label className="global-search"><Search size={17} /><input value={query} onChange={e => setQuery(e.target.value)} placeholder="Buscar projetos, desenhos, códigos..." /><kbd>Ctrl K</kbd></label>
            <button className="icon-button" aria-label="Notificações"><Bell size={18} /></button>
            <div className="avatar">EN</div>
          </div>
        </header>

        <div className="page-content">
          {isPreviewMode && <div className="preview-banner"><b>PRÉVIA VISUAL</b><span>Dados demonstrativos. Indexação, backup e restauração reais exigem o aplicativo desktop.</span></div>}
          {section === "Projetos" ? (<ProjectsPanel />) : section === "Explorador" ? (<ExplorerPanel />) : section === "Revisão" ? (<ReviewPanel />) : section === "Divergências" ? (<DiscrepanciesPanel />) : section === "Visão geral" ? (
            <>
              <div className="page-heading">
                <div><div className="eyebrow">CENTRAL DE CONTROLE</div><h1>Visão geral</h1><p>Encontre, organize e analise o conhecimento dos projetos da sua engenharia.</p></div>
                <div className="dashboard-actions">
                  <button className="secondary-button" onClick={() => void createBackup()} disabled={scanning}><Archive size={16} /> Backup</button>
                  <button className="secondary-button" onClick={() => void restoreBackup()} disabled={scanning}><ShieldCheck size={16} /> Restaurar</button>
                  <button className="primary-button" onClick={importFolder} disabled={scanning}><FolderOpen size={17} /> {scanning ? "Indexando..." : "Importar pasta"}</button>
                </div>
              </div>

              {(message || error) && <div className={error ? "notice error" : "notice"} role="status">{error || message}</div>}

              <section className="welcome-panel">
                <div className="welcome-copy"><div className="eyebrow light">BIBLIOTECA DE ENGENHARIA</div><h2>Seus projetos, organizados.<br /><em>Suas decisões, fundamentadas.</em></h2><p>Uma base técnica pesquisável, com histórico, rastreabilidade e análise de documentos.</p></div>
                <div className="welcome-art"><div className="art-ring ring-one" /><div className="art-ring ring-two" /><div className="art-file"><FileCheck2 size={38} /><span>GED</span></div><div className="art-tag tag-one">SHA-256</div><div className="art-tag tag-two">METADADOS</div></div>
              </section>

              <div className="section-title"><div><h3>Biblioteca</h3><p>Indicadores da base local</p></div><span className="muted-pill"><HardDrive size={13} /> Armazenamento local</span></div>
              <div className="stats-grid">
                <article className="stat-card"><div className="stat-icon blue"><Folder size={19} /></div><span>Pastas indexadas</span><strong>{stats.root_count}</strong><small>Diretórios adicionados à biblioteca</small></article>
                <article className="stat-card"><div className="stat-icon violet"><Files size={19} /></div><span>Documentos indexados</span><strong>{stats.file_count.toLocaleString("pt-BR")}</strong><small>Arquivos registrados no banco local</small></article>
                <article className="stat-card"><div className="stat-icon amber"><CircleAlert size={19} /></div><span>Grupos com hash duplicado</span><strong>{stats.duplicate_hash_groups}</strong><small>Conteúdos idênticos; revisar antes de excluir</small></article>
                <article className="stat-card"><div className="stat-icon green"><FileCheck2 size={19} /></div><span>Registros de carimbo</span><strong>{stats.drawing_metadata_count.toLocaleString("pt-BR")}</strong><small>Metadados importados de CSV reconhecido</small></article>
                <article className="stat-card"><div className="stat-icon amber"><CircleAlert size={19} /></div><span>Divergências importadas</span><strong>{stats.discrepancy_count.toLocaleString("pt-BR")}</strong><small>Registros de diagnóstico presentes nos CSVs</small></article>
              </div>

              <div className="lower-grid">
                <section className="surface-card"><div className="card-heading"><div><h3>Comece pela biblioteca</h3><p>Fluxo inicial recomendado</p></div><BookOpen size={18} /></div>
                  <div className="step-list"><div className="step"><span>01</span><div><b>Selecionar a pasta de projetos</b><small>Escolha a pasta raiz onde estão os arquivos históricos.</small></div><Clock3 size={16} /></div><div className="step"><span>02</span><div><b>Indexar documentos</b><small>O GED calcula hashes e registra os arquivos sem alterá-los.</small></div><Clock3 size={16} /></div><div className="step"><span>03</span><div><b>Revisar resultados</b><small>Consulte a quantidade de arquivos e possíveis duplicidades.</small></div><Clock3 size={16} /></div></div>
                </section>
                <section className="surface-card"><div className="card-heading"><div><h3>Integridade e confiança</h3><p>Princípios do GED</p></div><ShieldCheck size={18} /></div>
                  <div className="trust-item"><ShieldCheck size={17} /><div><b>Originais preservados</b><small>O indexador lê os arquivos e não os modifica.</small></div></div>
                  <div className="trust-item"><FileSearch size={17} /><div><b>Achados com evidências</b><small>Alertas automáticos não substituem validação técnica.</small></div></div>
                  <div className="trust-item"><Archive size={17} /><div><b>Rastreabilidade</b><small>Hash, caminho de origem e histórico de processamento.</small></div></div>
                </section>
              </div>
            </>
          ) : (
            <div className="empty-state"><div className="empty-icon"><SlidersHorizontal size={25} /></div><div className="eyebrow">MÓDULO EM CONSTRUÇÃO</div><h2>{section}</h2><p>Esta área será conectada ao banco local e aos serviços de indexação nas próximas entregas.</p><button className="secondary-button" onClick={() => setSection("Visão geral")}>Voltar à visão geral</button></div>
          )}
        </div>
        <footer className="statusbar"><span><span className="status-led" /> {isPreviewMode ? "Prévia visual" : "Aplicativo local"}</span><span>Windows 10+ · 64 bits</span><span>{isPreviewMode ? "Dados demonstrativos" : "Os arquivos originais não são modificados"}</span></footer>
      </main>
    </div>
  );
}

export default App;
