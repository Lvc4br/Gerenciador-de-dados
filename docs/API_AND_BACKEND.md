# Motor local, IPC e contratos internos

## 1. Papel do backend

No GED desktop, o backend é um **motor local empacotado**, não um servidor remoto obrigatório. A interface React comunica-se com Tauri e com o sidecar Python. O usuário não precisa instalar Python, Node ou banco de dados.

## 2. Contrato de comunicação

Operações rápidas (abrir diálogo de pasta, obter configuração, abrir arquivo/URL permitido) usam comandos Tauri. Operações longas (varrer diretório, extrair DWG/PDF, indexar, comparar e exportar) viram jobs no motor Python e emitem eventos de progresso.

Contrato conceitual:

- `start_scan(root_id, options) -> job_id`
- `get_job(job_id) -> JobStatus`
- `cancel_job(job_id) -> CancelResult`
- `search(query, filters, sort, cursor) -> SearchPage`
- `get_document(document_id) -> DocumentDetail`
- `get_preview(version_id) -> PreviewDescriptor`
- `compare_versions(version_a, version_b) -> ComparisonJob`
- `export_selection(selection, format, options) -> ExportJob`

Todos os comandos validam caminhos, IDs e permissões do perfil local. Não aceitar caminho arbitrário vindo da interface sem validação.

## 3. Estrutura do motor

```text
engine/
  ged_engine/
    main.py                # inicialização do sidecar
    ipc/                   # protocolo, autenticação efêmera e eventos
    core/
    catalog/
    ingestion/
    metadata/
    taxonomy/
    findings/
    search/
    comparison/
    recommendations/
    preview/
    exports/
    adapters/
    db/
    workers/
    tests/
```

## 4. Banco local

SQLite com foreign keys, WAL quando apropriado, FTS5, índices e migrações versionadas. SQLAlchemy 2 pode fornecer camada de persistência comum. O catálogo não deve guardar o conteúdo binário de todos os documentos em BLOB: os originais permanecem no filesystem e o banco guarda caminho, hash, tamanho e metadados.

Tabelas principais: projects, documents, document_locations, document_versions, extractions, metadata_values, taxonomies, findings, document_relations, ingestion_jobs, export_jobs, audit_events e settings.

## 5. Modelo de job

Estados: queued, running, succeeded, partial, failed, cancelled. Cada job guarda progresso, itens processados, erros por arquivo, duração, versão do pipeline e timestamps. Retomar varredura não deve duplicar registros. Cancelamento deve ser cooperativo e seguro.

## 6. Endpoints/ações de domínio

| Ação | Descrição |
|---|---|
| projects.list/create/update | Cadastro de obras |
| roots.add/list/verify | Pastas indexadas e verificação de caminhos |
| scan.start/status/cancel | Varredura e indexação |
| documents.list/detail | Explorador e detalhes |
| documents.reveal_in_folder | Abrir localização no Explorer do Windows |
| documents.open_original | Abrir no aplicativo associado, mediante ação do usuário |
| metadata.correct/verify | Correção e validação de metadados |
| findings.list/review | Revisão de divergências |
| search.query | Busca textual e filtros |
| comparison.start/result | Comparação |
| recommendations.query/feedback | Recomendações |
| exports.start/status/open | Exportação e localização do resultado |
| backup.create/restore | Backup e recuperação do catálogo |

## 7. Segurança de IPC

Se o sidecar usar HTTP loopback:
- Bind exclusivo em 127.0.0.1.
- Porta dinâmica e token aleatório por execução.
- Validar origem e limitar métodos/rotas.
- Não aceitar conexões de outras máquinas.
- Encerrar sidecar e invalidar token ao fechar o app.
- Nunca expor a porta em firewall/rede.

Preferir pipe/socket local quando viável. Segredos não devem aparecer em logs.

## 8. Testes

- Unitários para regras, parser, hash e normalização.
- Integração para SQLite e migrações.
- Fixtures reais anonimizadas: CSV AB20A, PDF, DOCX, XLSX e DWG de teste.
- Testar caminhos longos, caracteres acentuados, rede indisponível, disco removido, arquivos duplicados e corrompidos.
- Testar cancelamento, retomada e encerramento inesperado.
- Testar instalação limpa e atualização no Windows.


## 9. Sugestões de metadados pelo nome do arquivo (desktop atual)

O fluxo de revisão inicial é executado no backend Rust/Tauri, diretamente no banco SQLite do desktop. O parser Python/CLI continua disponível para diagnóstico e desenvolvimento, mas não é a fonte de dados da tela desktop.

Comandos Tauri:
- `list_filename_suggestions(status, search, limit, offset)`: retorna sugestões com nome e caminho do arquivo, projeto provável, disciplina, número, revisão, confiança e situação.
- `review_filename_suggestion(id, decision)`: registra `accepted` ou `rejected`; somente sugestões pendentes podem ser revisadas.

A tabela `filename_suggestions` possui chave única por arquivo e versão do parser. Reindexar com os mesmos valores preserva a decisão humana; quando os valores extraídos mudam, a situação volta para `pending`. A análise usa somente o nome do arquivo e nunca abre ou altera o original.

A aceitação atual registra a decisão, mas não promove os campos para os metadados oficiais nem cria associação automática a projeto. Essa promoção exige uma etapa própria de comparação com carimbo/cadastro e trilha de auditoria.


### Aplicação de metadados aceitos e auditoria

- `apply_accepted_filename_suggestion(id)`: permite promover uma sugestão já aceita para `confirmed_document_metadata`.
- A promoção ocorre em transação SQLite e mantém a origem em `source_suggestion_id`.
- `metadata_audit_log` registra criação/atualização, valores anteriores e novos em JSON, identificador local do ator e data/hora.
- O botão de aplicação é separado da aceitação. Assim, o engenheiro pode aceitar primeiro e aplicar em outro momento.
- A aplicação não cria vínculo de projeto, não altera o arquivo original e não substitui os dados importados dos CSVs.

A listagem de sugestões também consulta `drawing_metadata` por nome de arquivo e devolve `titleblock_internal_number`, `titleblock_artesp_number` e `titleblock_title` para conferência lado a lado. Quando não há correspondência importada, esses campos são nulos; isso não é tratado como divergência confirmada.


## Extração inicial de PDF (motor Python/CLI)

O módulo `ged_indexer.pdf.extract_pdf_text(path)` extrai texto incorporado em PDFs usando `pypdf`, sem rede e sem modificar o documento. A dependência é opcional (`pip install -e '.[pdf]'`); está incluída no extra de testes do CI.

Limites de proteção: 200 MB por arquivo, 5.000 páginas e 10 milhões de caracteres extraídos. PDFs criptografados sem senha são recusados. O extrator não executa OCR: arquivos digitalizados sem camada textual podem retornar texto vazio.

Comandos:
- `ged-indexer extract-pdf <file_id>`: localiza um PDF previamente catalogado, confere o SHA-256 atual contra a última varredura, extrai e salva o texto em `extracted_document_text`.
- `ged-indexer search-content "<termos>" --limit 50`: pesquisa o texto indexado pelo FTS5 e devolve caminho e trecho de contexto.

A tabela `document_text_fts` é mantida por triggers quando o conteúdo extraído é inserido, atualizado ou removido. O texto é separado dos arquivos originais. Este fluxo ainda pertence ao banco do motor Python; a integração com o banco SQLite Rust/Tauri e com a busca na interface desktop é trabalho pendente, não devendo ser confundida com uma função já disponível no executável.
