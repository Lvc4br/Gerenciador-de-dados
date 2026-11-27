import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive, ArrowDownToLine, Bell, BookOpen, ChevronRight,
  CircleAlert, Clock3, FileCheck2, FileSearch, Files, Folder, FolderOpen,
  HardDrive, LayoutDashboard, Search, Settings2, ShieldCheck, SlidersHorizontal,
  FileText, RefreshCw, ChevronLeft, ArrowUpDown
} from "lucide-react";

type Section = "Visão geral" | "Projetos" | "Explorador" | "Divergências" | "Exportações";
type DashboardStats = { root_count: number; file_count: number; duplicate_hash_groups: number; drawing_metadata_count: number; discrepancy_count: number };
type ScanSummary = { root: string; indexed: number; skipped: number; errors: number; duplicate_hash_groups: number };

const navigation: { label: Section; icon: typeof LayoutDashboard }[] = [
  { label: "Visão geral", icon: LayoutDashboard },
  { label: "Projetos", icon: Folder },
  { label: "Explorador", icon: Files },
  { label: "Divergências", icon: CircleAlert },
  { label: "Exportações", icon: ArrowDownToLine }
];

const emptyStats: DashboardStats = { root_count: 0, file_count: 0, duplicate_hash_groups: 0, drawing_metadata_count: 0, discrepancy_count: 0 };


type FileRecord = {
  id: number; filename: string; relative_path: string; extension: string;
  byte_size: number; modified_at: string; sha256: string; root_path: string;
};
type FilePage = { items: FileRecord[]; total: number };

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
  const [sortBy, setSortBy] = useState('path');
  const [ascending, setAscending] = useState(true);
  const [page, setPage] = useState(0);
  const [data, setData] = useState<FilePage>({ items: [], total: 0 });
  const [selected, setSelected] = useState<FileRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const pageSize = 100;

  async function loadFiles() {
    setLoading(true); setError('');
    try {
      const result = await invoke<FilePage>('list_files', {
        search, extension, sortBy, ascending, limit: pageSize, offset: page * pageSize
      });
      setData(result);
      setSelected(current => current && result.items.some(item => item.id === current.id)
        ? current : (result.items[0] ?? null));
    } catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }

  useEffect(() => { void loadFiles(); }, [search, extension, sortBy, ascending, page]);

  const totalPages = Math.max(1, Math.ceil(data.total / pageSize));
  return (
    <section className="explorer">
      <div className="page-heading explorer-heading">
        <div><div className="eyebrow">BIBLIOTECA LOCAL</div><h1>Explorador de arquivos</h1><p>Pesquise e inspecione os arquivos registrados nas pastas indexadas.</p></div>
        <button className="secondary-button" onClick={() => void loadFiles()} disabled={loading}><RefreshCw size={15} /> Atualizar</button>
      </div>
      <div className="explorer-toolbar">
        <label className="explorer-search"><Search size={16} /><input value={search} onChange={e => { setSearch(e.target.value); setPage(0); }} placeholder="Nome do arquivo ou caminho..." /></label>
        <label className="filter-control"><span>Extensão</span><input value={extension} onChange={e => { setExtension(e.target.value); setPage(0); }} placeholder="ex.: pdf, dwg" /></label>
        <label className="filter-control"><span>Ordenar por</span><select value={sortBy} onChange={e => setSortBy(e.target.value)}><option value="path">Caminho</option><option value="filename">Nome</option><option value="extension">Extensão</option><option value="size">Tamanho</option><option value="modified">Modificação</option></select></label>
        <button className="sort-button" title="Inverter ordenação" onClick={() => setAscending(value => !value)}><ArrowUpDown size={16} /> {ascending ? 'Crescente' : 'Decrescente'}</button>
      </div>
      <div className="explorer-summary"><span>{loading ? 'Consultando biblioteca...' : data.total.toLocaleString('pt-BR') + ' arquivo(s) encontrado(s)'}</span><span>Exibindo até 100 por página</span></div>
      {error && <div className="notice error">{error}</div>}
      <div className="explorer-layout">
        <div className="file-table-wrap">
          <table className="file-table">
            <thead><tr><th>Nome</th><th>Tipo</th><th>Tamanho</th><th>Última modificação</th></tr></thead>
            <tbody>
              {data.items.map(file => <tr key={file.id} className={selected?.id === file.id ? 'selected' : ''} onClick={() => setSelected(file)}>
                <td><div className="file-name"><FileText size={16} /><div><b>{file.filename}</b><small>{file.relative_path}</small></div></div></td>
                <td><span className="file-ext">{file.extension ? file.extension.toUpperCase() : 'SEM TIPO'}</span></td>
                <td>{formatBytes(file.byte_size)}</td>
                <td>{formatDate(file.modified_at)}</td>
              </tr>)}
              {!loading && data.items.length === 0 && <tr><td colSpan={4} className="table-empty">Nenhum arquivo encontrado. Importe uma pasta na Visão geral para começar.</td></tr>}
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
            <div className="detail-row hash-row"><span>SHA-256</span><code>{selected.sha256}</code></div>
            <p className="detail-note">Os dados acima foram registrados na indexação. A visualização do conteúdo e a extração de metadados serão adicionadas em uma próxima etapa.</p>
          </> : <p className="detail-note">Selecione um arquivo na lista para consultar suas propriedades.</p>}
        </aside>
      </div>
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

  async function importFolder() {
    setError("");
    const selected = await open({ directory: true, multiple: false, title: "Selecionar pasta de projetos" });
    if (typeof selected !== "string") return;
    setScanning(true);
    setMessage("Indexando arquivos. Os originais não serão modificados.");
    try {
      const result = await invoke<ScanSummary>("scan_folder", { folder: selected });
      await refreshStats();
      setMessage(`Indexação concluída: ${result.indexed} arquivo(s), ${result.skipped} ignorado(s), ${result.errors} erro(s). ${result.duplicate_hash_groups} grupo(s) de hashes duplicados na biblioteca.`);
    } catch (reason) {
      setError(String(reason));
      setMessage("");
    } finally {
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
              {label === "Divergências" && stats.duplicate_hash_groups > 0 && <i className="nav-dot" />}
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
          {section === "Explorador" ? (<ExplorerPanel />) : section === "Visão geral" ? (
            <>
              <div className="page-heading">
                <div><div className="eyebrow">CENTRAL DE CONTROLE</div><h1>Visão geral</h1><p>Encontre, organize e analise o conhecimento dos projetos da sua engenharia.</p></div>
                <button className="primary-button" onClick={importFolder} disabled={scanning}><FolderOpen size={17} /> {scanning ? "Indexando..." : "Importar pasta"}</button>
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
        <footer className="statusbar"><span><span className="status-led" /> Aplicativo local</span><span>Windows 10+ · 64 bits</span><span>Os arquivos originais não são modificados</span></footer>
      </main>
    </div>
  );
}

export default App;
