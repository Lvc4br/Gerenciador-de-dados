# Roadmap de desenvolvimento

## Fase 0 — Descoberta e validação (1–2 semanas)

- Entrevistar engenheiros, projetistas, gestores documentais e TI.
- Definir taxonomia, papéis, política de revisão e regras por disciplina.
- Ampliar amostra de arquivos reais (DWG de diferentes versões, PDF, DOCX, XLSX, IFC etc.).
- Confirmar requisitos de implantação, sigilo, licenças e volume.
- Criar conjunto ouro de documentos revisados por especialistas para medir precisão.

**Saída:** requisitos priorizados, taxonomia inicial, matriz de formatos e critérios de avaliação.

## Fase 1 — MVP documental (3–5 semanas)

- Login e workspaces.
- Cadastro de projetos e estrutura lógica de pastas.
- Upload individual e em lote.
- Hash, detecção de tipo, antivírus/quarentena e storage imutável.
- Indexação de CSV, TXT, PDF textual e metadados básicos de imagem.
- Explorador em lista/grade, breadcrumbs, preview PDF/imagem.
- Busca por nome, código, título e filtros.
- Exportação de metadados CSV/XLSX e download do original.
- Auditoria de upload, alteração de metadados e exportação.

**Aceite:** upload idempotente, original preservado, permissão aplicada no backend, busca reproduzível e exportação com manifesto.

## Fase 2 — Engenharia e divergências (4–6 semanas)

- Parser dos CSVs AB20A (UTF-8 BOM, delimitador ;).
- Adaptador CAD para extrair propriedades/textos/carimbo de DWG, conforme biblioteca/licença escolhida.
- Campos de desenho: número, título, revisão, ramo, rodovia, trecho, escala, ARTESP e código interno.
- Regras versionadas de divergência entre nome, carimbo e cadastro.
- Interface de revisão humana com evidências e auditoria.
- Detecção de duplicidade por hash e candidatos a duplicidade por metadados.

**Aceite:** reproduzir os 9 registros de divergências da amostra de referência quando os dados de entrada correspondentes estiverem disponíveis; explicar cada achado e sua evidência. Não afirmar leitura dos DWGs apenas com base nos CSVs.

## Fase 3 — Visualização e comparação (4–8 semanas)

- Adaptadores para PDF, imagens e formatos Office.
- Avaliar visualizador CAD e BIM (open source, SDK comercial ou serviço gerenciado).
- Histórico de versões e comparação de metadados.
- Comparação PDF por texto/visual.
- Comparação CAD/BIM apenas com suporte validado.
- Exportação de relatórios de comparação.

**Aceite:** cada formato declara capacidades e limitações; resultados incluem versão da ferramenta e parâmetros de comparação.

## Fase 4 — Busca avançada e recomendações (3–5 semanas)

- Busca textual com facetas.
- Busca semântica com embeddings.
- Similaridade técnica baseada em atributos normalizados.
- Explicabilidade do ranking.
- Feedback de relevância e conjunto de testes de busca.
- Recomendações respeitam autorização e sigilo.

**Aceite:** métricas de precisão/recall em conjunto de consulta revisado por engenheiros; nenhum resultado fora das permissões do usuário.

## Fase 5 — Escala, integrações e governança (contínua)

- OpenSearch, filas escaláveis e workers horizontais.
- Integração com SharePoint, Autodesk Construction Cloud, armazenamento corporativo ou GED existente conforme demanda.
- SSO/MFA, retenção, legal hold, backups e recuperação testada.
- APIs de integração e webhooks.
- Dashboards operacionais e métricas de qualidade.
- Suporte a novos formatos e normas internas.

## Backlog técnico transversal

- Migrações de banco e seeds de taxonomia.
- Contratos OpenAPI e versionamento da API.
- CI: lint, type-check, testes unitários, integração e segurança.
- Testes com arquivos malformados, enormes e arquivos compactados maliciosos.
- Testes de concorrência, idempotência e retomada de jobs.
- Testes de isolamento entre workspaces.
- SBOM, scan de dependências, secret scanning e política de atualização.
- Backup/restore automatizado e disaster recovery.
- Documentação de instalação, operação e suporte.

## Indicadores de sucesso

- Tempo mediano para localizar um projeto de referência.
- Percentual de arquivos com metadados úteis extraídos.
- Precisão e recall das regras de divergência.
- Taxa de falsos positivos por regra.
- Taxa de recomendação considerada útil.
- Percentual de uploads processados sem intervenção.
- Tempo de indexação por formato e tamanho.
- Falhas de preview/exportação.
- Incidentes de permissão (meta: zero).

## Riscos e mitigação

| Risco | Mitigação |
|---|---|
| Formatos CAD proprietários/licenças | Adaptadores e prova técnica antes de prometer suporte |
| Carimbos variam por empresa | Templates configuráveis e revisão humana |
| OCR/IA alucina valores | Evidência por campo, confiança e confirmação |
| Falsos positivos | Regras versionadas, conjunto ouro e feedback |
| Arquivos maliciosos | Quarentena, antivírus e workers isolados |
| Vazamento entre clientes | Isolamento por workspace e testes de autorização |
| Exportação perde fidelidade | Distinguir exportar original de converter/gerar novo arquivo |
| Dados históricos inconsistentes | Preservar bruto e normalizado, sem correção destrutiva |

## Definição de pronto

Uma funcionalidade está pronta quando tem critérios de aceite, autorização, logs/auditoria, tratamento de falhas, testes automatizados, documentação e comportamento explícito para dados ausentes ou não suportados.
