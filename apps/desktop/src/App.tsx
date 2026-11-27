import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive, ArrowDownToLine, Bell, BookOpen, ChevronRight,
  CircleAlert, Clock3, FileCheck2, FileSearch, Files, Folder, FolderOpen,
  HardDrive, LayoutDashboard, Search, Settings2, ShieldCheck, SlidersHorizontal
} from "lucide-react";

type Section = "Visão geral" | "Projetos" | "Explorador" | "Divergências" | "Exportações";
type DashboardStats = { root_count: number; file_count: number; duplicate_hash_groups: number };
type ScanSummary = { root: string; indexed: number; skipped: number; errors: number; duplicate_hash_groups: number };

const navigation: { label: Section; icon: typeof LayoutDashboard }[] = [
  { label: "Visão geral", icon: LayoutDashboard },
  { label: "Projetos", icon: Folder },
  { label: "Explorador", icon: Files },
  { label: "Divergências", icon: CircleAlert },
  { label: "Exportações", icon: ArrowDownToLine }
];

const emptyStats: DashboardStats = { root_count: 0, file_count: 0, duplicate_hash_groups: 0 };

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
          {section === "Visão geral" ? (
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
                <article className="stat-card"><div className="stat-icon green"><ShieldCheck size={19} /></div><span>Integridade</span><strong>SHA-256</strong><small>Hash calculado durante a indexação</small></article>
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
