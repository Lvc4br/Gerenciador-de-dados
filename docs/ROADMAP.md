# Roadmap — aplicativo desktop GED para Windows

## Fase 0 — Prova de conceito desktop

- Confirmar Windows 10/11 e arquitetura x64 como alvo inicial.
- Criar shell Tauri 2 + React + TypeScript.
- Empacotar motor Python como sidecar.
- Testar acesso a pastas locais, caminhos longos, acentos e compartilhamentos de rede.
- Definir armazenamento do banco SQLite e diretórios de cache.
- Validar licenças e bibliotecas de DWG/PDF/Office/IFC.

**Saída:** protótipo instalável que abre, seleciona pastas e inicia o motor local.

## Fase 1 — MVP documental executável

- Instalador Windows (.exe ou .msi) e execução sem dependências instaladas pelo usuário.
- Perfil local e configurações.
- Cadastro de projetos e raízes de biblioteca.
- Varredura recursiva, importação e indexação de pastas.
- Hash SHA-256, deduplicação e preservação dos originais.
- SQLite + FTS5 e migrações versionadas (baseline, índices de catálogo e índice textual já registrados).
- Leitura de CSV e TXT no catálogo desktop; extração de PDF textual disponível inicialmente no motor Python/CLI, pendente integração ao desktop.
- Explorer com árvore, lista/grade, breadcrumbs, ordenação, filtros e preview.
- Busca e exportação CSV/XLSX.
- Backup e restauração do catálogo.

**Aceite:** instalar em Windows limpo, operar offline, indexar a amostra AB20A e manter originais intactos.

## Fase 1A — Catálogo de projetos

- Modelo lógico de projetos separado das raízes físicas indexadas.
- Cadastro com código único, nome, cliente, descrição e status.
- Vínculo reutilizável entre projetos e raízes existentes.
- Disciplinas configuráveis por projeto.
- Associação de documentos com origem e evidência, sem mover ou alterar originais.
- Testes de migração, integridade referencial e persistência offline.

**Especificação:** [PROJECT_CATALOG.md](PROJECT_CATALOG.md).

### Etapa inicial — sugestões pelo nome do arquivo

- Implementado parser conservador de prefixo de projeto, disciplina, número de desenho e revisão.
- Resultados identificados como sugestões com indicador de confiança e origem `filename`.
- Comando local de diagnóstico: `ged-indexer analyze-name "AB20A-EST-001_REV02.dwg"`.
- O parser Python oferece diagnóstico via CLI; o desktop possui parser conservador equivalente em Rust para que a funcionalidade opere diretamente no SQLite usado pelo aplicativo.
- Durante a varredura desktop, sugestões com correspondência são persistidas em `filename_suggestions`, separadas dos vínculos de projeto e dos metadados importados do carimbo.
- A tela **Revisão** lista sugestões por situação, permite pesquisa e registra decisões `accepted` ou `rejected`.
- A tela exibe também os números e o título importados do carimbo quando encontra correspondência pelo nome do arquivo.
- Uma nova varredura com o mesmo resultado preserva a decisão humana; se os metadados identificados mudarem, a sugestão retorna para `pending`.
- Sugestões aceitas podem ser aplicadas explicitamente a `confirmed_document_metadata`; a operação registra valores anteriores/novos em `metadata_audit_log` e é idempotente para a mesma sugestão.
- A aplicação não altera CSVs importados, não modifica o original e não cria vínculo automático de projeto.
- Nenhum arquivo é aberto, renomeado, movido ou alterado por essa análise.
- O módulo Python inicial de PDF usa `pypdf`, extrai texto por página offline e impõe limites de tamanho (200 MB), páginas (5.000) e caracteres (10 milhões); PDFs protegidos por senha são recusados sem tentativa de quebra.
- O texto extraído é armazenado separado dos originais em `extracted_document_text` e indexado em FTS5 (`document_text_fts`); o CLI oferece `extract-pdf` e `search-content`.
- Esta etapa é um protótipo do motor Python/CLI: ainda não está conectado ao backend Rust/Tauri nem à tela de pesquisa do desktop. PDFs digitalizados sem camada de texto exigem OCR, que não está incluído nesta etapa.

- A comparação automatizada com os dados do carimbo e a aplicação controlada de valores aceitos permanecem etapas futuras.

## Fase 2 — Engenharia e divergências

- Parser CSV AB20A (UTF-8 BOM e delimitador ponto e vírgula).
- Extrair número, título, revisão, ramo, rodovia, trecho, escala, ARTESP e código interno.
- Adaptador DWG após prova técnica e validação de licença.
- Regras entre nome, carimbo, cadastro e revisões.
- Tela de revisão humana com evidências e auditoria.
- Detecção de duplicidade por hash e candidatos por metadados.

**Aceite:** reproduzir os 9 registros de divergências da amostra quando os dados correspondentes estiverem disponíveis; mostrar evidência e origem.

## Fase 3 — Visualização e comparação

- Preview PDF, imagens e Office.
- Avaliar visualizador CAD/BIM nativo ou baseado em SDK.
- Histórico de versões.
- Comparação de metadados, PDF e desenhos conforme capacidades verificadas.
- Relatórios de comparação.

## Fase 4 — Busca avançada e recomendações

- Busca híbrida local.
- Similaridade técnica por atributos.
- Embeddings locais opcionais.
- Explicabilidade e feedback de relevância.
- Benchmark de consultas revisado por engenheiros.

## Fase 5 — Distribuição e maturidade

- Assinatura de código e instalador.
- Atualização segura e rollback.
- Logs, crash reports com consentimento e diagnóstico local.
- Backup agendado e recuperação testada.
- Otimização para grandes bibliotecas.
- Sincronização multiusuário opcional, com servidor corporativo separado.
- Integrações com SharePoint/Autodesk Construction Cloud somente se houver demanda e autorização.

## Indicadores de sucesso

- Tempo mediano para localizar projeto de referência.
- Percentual de metadados úteis extraídos.
- Precisão/recall e falsos positivos das regras.
- Taxa de recomendação considerada útil.
- Tempo de indexação por formato/tamanho.
- Falhas de preview/exportação.
- Uso de memória e CPU em bibliotecas grandes.
- Taxa de recuperação após interrupção.
- Incidentes de perda/alteração de originais (meta: zero).

## Riscos

| Risco | Mitigação |
|---|---|
| DWG/RVT proprietários | Prova técnica e licença antes de prometer suporte |
| Bibliotecas grandes | Indexação incremental, streaming e concorrência configurável |
| Caminhos removidos | Verificador de raízes e reparo de localização |
| Arquivos maliciosos | Processos isolados e limites de recursos |
| Falsos positivos | Regras versionadas, conjunto ouro e revisão humana |
| Corrupção do catálogo | Transações, backup e recuperação |
| Atualização do aplicativo | Assinatura, validação e rollback |
| Envio indevido de documentos | Processamento local por padrão e consentimento explícito para nuvem |

## Definição de pronto

Uma versão só é considerada pronta quando o instalador funciona em Windows limpo, operações essenciais funcionam offline, os originais são preservados, os jobs são recuperáveis, há testes automatizados, backup/restauração e documentação de instalação.
