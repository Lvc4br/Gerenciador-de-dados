# Segurança, privacidade e garantia da qualidade

## 1. Modelo de ameaça

Proteger o GED contra:
- acesso indevido a projetos/documentos de outro workspace;
- upload de malware, macro, arquivo compactado malicioso ou parser exploit;
- documentos excessivamente grandes ou construídos para esgotar CPU/memória;
- exposição de URLs de storage e previews;
- exfiltração por exportação em lote;
- prompt injection em conteúdo documental processado por IA;
- adulteração de originais, metadados verificados e trilhas de auditoria;
- abuso de credenciais e automações.

## 2. Controles mínimos

- OIDC/OAuth2, MFA conforme política corporativa e sessões revogáveis.
- RBAC + escopo de projeto/workspace; autorização no servidor em cada operação.
- Isolamento de tenants testado automaticamente.
- Criptografia TLS e criptografia em repouso.
- Buckets privados; URLs assinadas com expiração curta.
- Quarentena, antivírus e sandbox para parsers/conversores.
- Limites de upload, descompactação, páginas, entidades CAD, tempo e memória.
- Bloquear macros e execução de conteúdo ativo.
- Política CORS restrita, CSRF quando aplicável, CSP e headers de segurança.
- Rate limiting e proteção contra enumeração de IDs.
- Secrets manager; secret scanning em CI.
- Backups criptografados e testes periódicos de restauração.
- Auditoria append-only para operações sensíveis.
- Dependabot/scan de dependências, SAST e SBOM.
- Retenção, descarte e legal hold configuráveis.

## 3. IA e conteúdo não confiável

Arquivos e textos extraídos são dados, nunca instruções para o agente. Modelos não podem executar ações, alterar documentos ou acessar dados sem autorização. Toda saída de IA deve:
- apontar a evidência original;
- distinguir extração, inferência e recomendação;
- ter confiança calibrada ou informar que não há score confiável;
- permitir revisão humana;
- não afirmar conformidade normativa sem regra formal, versão da norma e validação competente;
- respeitar filtros de acesso no retrieval e na geração.

## 4. Plano de testes

### Unitários
- normalização de identificadores e revisões;
- parsers e adaptadores;
- regras de divergência;
- ranking e filtros;
- política de exportação;
- controle de permissões.

### Integração
- banco, storage, fila e workers;
- idempotência e reprocessamento;
- upload parcial e retomada;
- geração de previews;
- exportação com manifesto;
- falhas de dependências externas.

### Segurança
- IDOR e cross-tenant;
- path traversal e zip bomb;
- MIME spoofing;
- arquivos malformados;
- SSRF em conversores e URLs externas;
- XSS em nomes e textos extraídos;
- abuso de exportação e URLs assinadas;
- prompt injection em documentos.

### Aceitação com engenheiros
- recuperar documentos conhecidos por código, título e contexto;
- conferir divergências esperadas;
- medir falsos positivos e falsos negativos;
- validar se as recomendações são realmente úteis;
- avaliar se o preview preserva contexto e legibilidade;
- conferir que o sistema não confunde revisão, emissão e aprovação.

## 5. Métricas de qualidade

- Precision/recall por regra de divergência.
- Taxa de campos extraídos corretamente por formato.
- Percentual de metadados com evidência navegável.
- Recall@k e NDCG@k para busca/recomendação.
- Tempo p50/p95 de busca e preview.
- Taxa de jobs presos, falhas e reprocessamentos.
- RPO/RTO definidos com a organização e testados.
- Incidentes de autorização: meta zero.
