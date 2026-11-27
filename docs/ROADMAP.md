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
- SQLite + FTS5 e migrações.
- Leitura de CSV, TXT e PDF textual.
- Explorer com árvore, lista/grade, breadcrumbs, ordenação, filtros e preview.
- Busca e exportação CSV/XLSX.
- Backup e restauração do catálogo.

**Aceite:** instalar em Windows limpo, operar offline, indexar a amostra AB20A e manter originais intactos.

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
