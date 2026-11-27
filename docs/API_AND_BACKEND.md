# Backend e contratos da API

## 1. Organização recomendada

Monólito modular em Python/FastAPI. Cada módulo contém router, schemas, service, repository e políticas de autorização. Regras de domínio não devem ficar nos endpoints.

```text
backend/
  app/
    main.py
    core/              # config, auth, permissions, errors, logging
    modules/
      identity/
      workspaces/
      projects/
      documents/
      ingestion/
      metadata/
      taxonomy/
      search/
      findings/
      comparison/
      recommendations/
      preview/
      exports/
      audit/
    adapters/
      base.py
      csv_adapter.py
      pdf_adapter.py
      image_adapter.py
      office_adapter.py
      cad_adapter.py
      ifc_adapter.py
    workers/
      tasks/
      pipelines/
    db/
      models/
      migrations/
    tests/
      unit/
      integration/
      fixtures/
  alembic/
  pyproject.toml
  Dockerfile
```

## 2. Convenções

- Prefixo: `/api/v1`.
- JSON UTF-8; datas ISO-8601 UTC.
- UUID como identificador público.
- Paginação por cursor para listagens grandes.
- Filtros explícitos e ordenação por campos permitidos (allowlist).
- Erros com `code`, `message`, `details`, `correlation_id`.
- Idempotency-Key para upload, ingestão e exportação.
- OpenAPI gerado pela aplicação e versionado em CI.
- Todas as rotas validam workspace, projeto e permissão no backend.

## 3. Endpoints previstos

| Método | Endpoint | Uso |
|---|---|---|
| GET/POST | /workspaces | Listar/criar workspaces |
| GET/PATCH | /projects | Listar e atualizar projetos |
| POST | /projects | Criar projeto |
| GET | /projects/{id} | Detalhes do projeto |
| GET/POST | /projects/{id}/documents | Listar/adicionar documentos |
| POST | /documents/upload-sessions | Criar sessão de upload |
| POST | /documents/{id}/versions | Registrar nova versão |
| GET | /documents/{id} | Detalhes do documento |
| GET | /documents/{id}/versions | Histórico de versões |
| GET | /versions/{id}/preview | Obter preview autorizado |
| GET | /versions/{id}/download | Download do original |
| POST | /ingestion-jobs | Criar job de ingestão em lote |
| GET | /jobs/{id} | Progresso e erros do job |
| GET | /search | Busca textual/facetada |
| GET | /findings | Listar divergências |
| POST | /findings/{id}/review | Confirmar/rejeitar/resolver achado |
| POST | /comparisons | Criar comparação |
| GET | /comparisons/{id} | Resultado da comparação |
| GET | /recommendations | Projetos/documentos recomendados |
| POST | /recommendations/feedback | Feedback de relevância |
| POST | /exports | Solicitar exportação |
| GET | /exports/{id} | Status e manifesto |
| GET | /audit-events | Auditoria conforme permissão |

## 4. Exemplos de contrato

### Criar projeto

```json
{
  "code": "SP330-TR-01",
  "name": "Trevo Anhanguera — referência",
  "project_type": "rodoviario",
  "discipline": ["estruturas", "geometria"],
  "stage": "executivo",
  "location": {
    "municipality": "São Paulo",
    "state_code": "SP",
    "country_code": "BR"
  }
}
```

### Resultado de busca

```json
{
  "items": [
    {
      "document_id": "uuid",
      "version_id": "uuid",
      "name": "desenho-ponte.dwg",
      "title": "Ponte — implantação",
      "discipline": "estruturas",
      "revision": "R4",
      "matched_by": ["title", "drawing_number"],
      "score": 0.91
    }
  ],
  "facets": {
    "discipline": [{"value": "estruturas", "count": 18}],
    "format": [{"value": "dwg", "count": 12}]
  },
  "next_cursor": null
}
```

Scores devem ser calibrados e não representam probabilidade de correção técnica.

### Achado de divergência

```json
{
  "rule_code": "DRAWING_NUMBER_MISMATCH",
  "severity": "high",
  "status": "open",
  "title": "Número do desenho divergente",
  "evidence": {
    "filename": {"value": "E-266", "locator": {"kind": "filename"}},
    "title_block": {"value": "E-239", "locator": {"kind": "title_block", "sheet": 1}}
  },
  "confidence": 0.98,
  "requires_human_review": true
}
```

## 5. Upload seguro e confiável

1. API cria sessão e reserva cota.
2. Cliente envia arquivo para área de quarentena (preferir upload multipart/resumível).
3. Worker valida tamanho, assinatura binária, MIME, extensão e antivírus.
4. Calcula SHA-256; detecta duplicidade e política de retenção.
5. Move/copia para storage original imutável.
6. Cria versão e job de ingestão com chave idempotente.
7. Worker extrai metadados e gera derivados em bucket separado.
8. API retorna status; frontend acompanha por polling ou SSE.

Não aceitar caminho de arquivo arbitrário do cliente nem confiar em nome, MIME ou hash enviados pelo navegador.

## 6. Modelo de autorização

Papéis sugeridos: owner, admin, engineer, reviewer, archivist, viewer. As permissões devem ser granulares:
- project.read / project.manage
- document.read / document.upload / document.manage
- metadata.edit / metadata.verify
- finding.read / finding.review
- export.create / export.download
- audit.read / workspace.manage

A autorização deve considerar workspace, projeto, classificação/sensibilidade e vínculo do usuário. Aplicar filtro de acesso antes de busca textual ou vetorial para impedir vazamento por resultados, contagens ou sugestões.

## 7. Processamento assíncrono

Estados de job: queued, running, succeeded, partial, failed, cancelled. Registrar tentativas, progresso, itens ignorados, erros seguros e duração. Usar retry com backoff somente para erros transitórios; erros de formato inválido não devem entrar em retry infinito.

## 8. Regras de qualidade do código

- Pydantic para validação de contratos.
- SQLAlchemy 2 + Alembic para persistência/migrações.
- Camada de serviço para regras de negócio.
- Repository para consultas.
- Testes de autorização por endpoint.
- Testes com fixtures reais anonimizadas e arquivos malformados.
- Não registrar conteúdo de documento nem credenciais em logs.
