# Arquitetura desktop do GED

## 1. Decisão principal

O GED é um **aplicativo desktop Windows**, instalado localmente e distribuído como `.exe`/instalador. Não é um SaaS nem uma aplicação cujo uso normal dependa do navegador.

### Stack

- Tauri 2: shell desktop, janela, menus, permissões e empacotamento.
- React + TypeScript: interface.
- Python sidecar empacotado: extração, OCR, classificação, regras e adaptadores técnicos.
- SQLite: catálogo, metadados, jobs, auditoria e índice FTS5.
- Sistema de arquivos: originais e derivados locais.
- Serviços externos opcionais: somente para sincronização, IA remota ou integrações explicitamente habilitadas.

O aplicativo deve operar offline para importação local, pesquisa, classificação, análise, visualização suportada e exportação.

## 2. Diagrama de contexto

```mermaid
flowchart TB
  U[Engenheiro] --> APP[GED Desktop Windows]
  APP --> UI[React + TypeScript]
  UI --> TAURI[Tauri / comandos nativos]
  TAURI --> CORE[Python sidecar empacotado]
  CORE --> DB[(SQLite + FTS5)]
  CORE --> FS[Pastas locais / rede autorizada]
  CORE --> PARSERS[Adaptadores PDF, Office, CSV, CAD, BIM]
  CORE --> RULES[Classificação e divergências]
  CORE --> SEARCH[Busca e recomendação local]
  CORE --> PREVIEW[Previews e miniaturas]
  CORE --> EXPORT[Exportação e relatórios]
  APP -. opcional e consentido .-> SYNC[Servidor corporativo futuro]
```

## 3. Organização do código

```text
gerenciador-de-dados/
  apps/
    desktop/                 # Tauri 2 + React + TypeScript
      src/                   # telas, componentes, estado
      src-tauri/             # Rust commands, permissões, bundler
  engine/
    ged_engine/              # Python sidecar
      core/                  # configuração, logging, erros
      catalog/               # projetos, documentos, versões
      ingestion/             # varredura, importação, jobs
      metadata/              # extração e proveniência
      taxonomy/              # classificação
      findings/              # regras e divergências
      search/                # FTS e similaridade
      preview/               # derivados seguros
      export/                # exportadores e manifestos
      adapters/              # csv, pdf, office, image, cad, ifc
      db/                    # SQLite, modelos e migrações
      tests/
  installer/                 # ícones, configuração e scripts
  docs/
```

## 4. Fluxo de importação local

```mermaid
flowchart TD
  A[Usuário seleciona pasta/arquivos] --> B[Validar diretórios e permissões]
  B --> C[Varredura recursiva controlada]
  C --> D[Detectar formato e calcular SHA-256]
  D --> E{Arquivo já catalogado?}
  E -- Sim --> F[Registrar ocorrência/localização ou duplicidade]
  E -- Não --> G[Registrar documento e versão]
  G --> H[Enfileirar job local]
  H --> I[Extrair texto, propriedades e estrutura]
  I --> J[Normalizar metadados preservando valores brutos]
  J --> K[Classificar e executar regras]
  K --> L[Gerar preview e indexar]
  L --> M[Disponibilizar no Explorer e na busca]
  F --> M
```

Não copiar nem mover arquivos por padrão: catalogar no local de origem e manter o original intacto. Oferecer cópia para uma biblioteca gerenciada apenas como ação explícita.

## 5. Domínios

- Workspace/Profile: perfil local e configuração.
- Project Registry: obras, contratos, locais e disciplinas.
- Document Catalog: documentos, versões, caminhos e hashes.
- Ingestion: varredura, detecção, extração e jobs.
- Metadata & Taxonomy: campos normalizados, taxonomia e proveniência.
- Search: texto completo, filtros e similaridade local.
- Quality & Findings: divergências, duplicidades e revisão humana.
- Comparison: diferenças entre versões/documentos.
- Recommendation: referências semelhantes e justificativas.
- Preview: miniaturas e derivados.
- Export & Reporting: relatórios, planilhas e pacotes.
- Audit: trilha local das operações.

## 6. Banco e armazenamento

SQLite é a fonte primária no MVP. Usar transações, WAL quando adequado, foreign keys, índices, FTS5 e migrações versionadas. O banco fica em diretório de dados do usuário, separado do diretório de instalação.

Os arquivos originais permanecem nos caminhos escolhidos pelo usuário. O GED guarda metadados, hash, caminho, tamanho, datas e resultados de extração. Previews, thumbnails, cache e exports ficam em diretórios próprios do GED. Deve existir ferramenta para relocalizar raiz de biblioteca e reparar caminhos quebrados.

Para grandes volumes, evitar carregar arquivos inteiros na memória. Processar em streaming e limitar concorrência configurável.

## 7. Comunicação entre interface e motor

Preferir comandos Tauri para operações nativas pequenas e um canal IPC local restrito para jobs longos do Python sidecar. Se usar HTTP loopback, vincular somente a localhost, autenticar com token efêmero por execução, escolher porta dinâmica, validar origem e encerrar o processo ao fechar o aplicativo. Nunca expor uma API aberta na rede local por padrão.

O sidecar deve iniciar e parar com o app, reportar saúde/versão e comunicar progresso por eventos. Falhas do motor não podem corromper o catálogo.

## 8. Divergências

1. Extrair valores candidatos de nome, carimbo, propriedades internas e tabelas.
2. Preservar cada valor com origem, método, confiança e localização (página/folha/entidade/coordenadas).
3. Normalizar apenas para comparar; preservar o bruto.
4. Aplicar regras versionadas por tipo de documento/disciplina.
5. Criar achados com severidade, evidência e estado.
6. Permitir confirmar, rejeitar, resolver ou aceitar risco com justificativa.
7. Auditar decisões.

Divergência é alerta para revisão, não conclusão automática de erro ou não conformidade.

## 9. Busca e recomendação local

- FTS5 para nome, título, conteúdo e identificadores.
- Filtros estruturados: obra, rodovia, trecho, ramo, disciplina, tipo, etapa, revisão, data, escala e formato.
- Similaridade lexical e técnica por atributos normalizados.
- Embeddings locais opcionais, ativados conforme hardware e licença/modelo.
- Ranking explicável: exibir atributos coincidentes, diferenças e dados ausentes.

Nunca recomendar projeto como tecnicamente aplicável apenas por similaridade. A decisão é do engenheiro responsável.

## 10. Formatos CAD/BIM

Implementar adaptadores com capacidades declaradas por formato/versão. DWG e RVT podem exigir SDKs ou licenças comerciais. Fazer prova de conceito com os DWGs reais antes de escolher biblioteca. Não converter silenciosamente nem alterar o original.

## 11. Segurança desktop

- Permissões nativas Tauri com allowlist mínima.
- Acesso a pastas somente após seleção/autorização do usuário.
- Não executar macros, scripts ou conteúdo ativo dos documentos.
- Processar arquivos potencialmente maliciosos em subprocessos isolados, com limites de tempo/memória.
- Guardar tokens/segredos no armazenamento seguro do sistema operacional quando necessário.
- Não enviar documentos para nuvem sem consentimento explícito.
- Logs sem conteúdo sensível; auditoria de importação, edição, exportação e exclusão.
- Backup do catálogo e opção de backup dos arquivos gerenciados.
- Assinatura do instalador e atualização segura antes de distribuição ampla.

## 12. Evolução multiusuário

A primeira versão é local e single-user/perfil local. Para colaboração futura, adicionar serviço de sincronização e API corporativa como módulo opcional, com conflitos de sincronização, identidade central e PostgreSQL no servidor. Não introduzir PostgreSQL, Redis, MinIO ou dependência de nuvem no MVP desktop.
