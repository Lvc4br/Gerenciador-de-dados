# GED — Gerenciador de Engenharia e Documentos

Aplicativo **desktop para Windows**, distribuído como instalador e executável, para organizar, localizar, analisar, comparar e reutilizar projetos anteriores de engenharia civil.

> **Objetivo:** transformar arquivos históricos dispersos em uma base documental local, pesquisável, rastreável e confiável para apoiar engenheiros.

## Produto e plataforma

O GED é um programa instalado no computador do usuário. A operação principal deve funcionar **offline**, sem depender de navegador, servidor remoto ou conexão com a internet. O alvo inicial é Windows 10 ou superior, x64, com um engenheiro usando um computador por vez.

A arquitetura poderá evoluir para sincronização opcional com servidor corporativo, mas isso não é requisito do MVP.

## Visão do produto

- Explorador semelhante ao Windows Explorer, com lista, propriedades, filtros e pesquisa.
- Importação/indexação de pastas inteiras, sem modificar os originais.
- Extração de metadados de arquivos, carimbos, textos, tabelas e propriedades CAD/BIM.
- Classificação assistida e taxonomia configurável.
- Busca textual, filtros, similaridade técnica e recomendações explicáveis.
- Detecção de divergências, duplicidades, lacunas e conflitos.
- Comparação de revisões e emissão de relatórios.
- Exportação de originais, metadados, resultados e pacotes.
- Banco local, backup e auditoria.

## Stack recomendada

| Camada | Tecnologia |
|---|---|
| Aplicativo Windows | Tauri 2 + Rust |
| Interface | React + TypeScript |
| Processamento documental | Python empacotado como sidecar (PyInstaller), quando integrado |
| Banco local | SQLite + FTS5 |
| PDF | PDF.js |
| OCR | Tesseract como primeira prova de conceito |
| CSV/Office | Bibliotecas Python especializadas por formato |
| CAD/BIM | Adaptadores isolados; validar SDK, versões e licenças |
| Testes | pytest, testes Rust e testes de interface |
| CI e instaladores | GitHub Actions em Windows x64; MSI/NSIS |

**Divisão recomendada:** Rust/Tauri para desktop, permissões e operações locais; React/TypeScript para interface; Python para extração e análise documental; SQLite para catálogo e busca local. Não adicionar PostgreSQL, Redis ou nuvem no MVP single-user.

## Evidência inicial: conjunto AB20A

Foi inspecionado o pacote de amostra `AB20A.zip`, com:
- 3 desenhos DWG;
- `titulos_carimbo.csv`: 1.336 linhas e 14 colunas;
- `divergencias_numero_carimbo.csv`: 9 linhas e 11 colunas.

Os CSVs usam delimitador ponto e vírgula e codificação UTF-8 com BOM. Entre os campos estão arquivo, ramo, candidato a título, rodovia, trecho, escala, desenho ARTESP, desenho interno, número no nome, número no carimbo, revisão, diagnóstico e caminho original.

A amostra demonstra um caso de uso concreto: conferir se identificadores e títulos associados ao desenho são coerentes. Os CSVs são evidência de metadados extraídos; não substituem a inspeção integral dos DWGs nem uma validação técnica do projeto.

## Documentação

- [Arquitetura desktop](docs/ARCHITECTURE.md)
- [Modelo de dados](docs/DATA_MODEL.md)
- [Desenvolvimento](docs/DEVELOPMENT.md)
- [Plano de desenvolvimento](docs/ROADMAP.md)
- [Backend e comunicação local](docs/API_AND_BACKEND.md)
- [Auditoria técnica e recomendações de stack](docs/TECHNICAL_AUDIT.md)

## Princípios não negociáveis

1. Nunca alterar o arquivo original durante ingestão, leitura ou análise.
2. Calcular hash SHA-256 e preservar proveniência, datas e origem.
3. Não confundir alerta automatizado com não conformidade de engenharia.
4. Mostrar evidências, regra aplicada e versão do analisador em cada achado.
5. Não sobrescrever revisões: cada versão deve ser preservada.
6. Permitir correção humana, registrar quem corrigiu e preservar o valor extraído.
7. Respeitar licenças de bibliotecas, conversores e formatos proprietários.
8. Restringir acesso a arquivos locais aos diretórios autorizados pelo usuário.
9. Não enviar documentos para serviços externos/IA em nuvem sem consentimento explícito.
10. Tratar arquivos não suportados ou corrompidos como falhas explícitas, nunca como sucesso silencioso.

## Estado atual da implementação

A branch `feature/mvp-core-indexer` contém:
- shell desktop Tauri 2 com interface React/TypeScript;
- seletor nativo de pastas;
- indexador Rust inicial com SHA-256 e SQLite local, executado fora da thread da interface;
- registro de execuções de varredura e reconciliação de arquivos ausentes, sem apagar os registros;
- estados de arquivo indexado, ignorado, com erro ou ausente;
- histórico de versões por hash SHA-256, tamanho e datas, sem cópia binária do conteúdo;
- backup consistente do banco SQLite com verificação de integridade;
- restauração de backup GED validado, criando uma cópia automática do banco atual antes da substituição;
- indicadores de pastas, arquivos indexados e grupos de hashes duplicados;
- Explorador conectado ao banco, com pesquisa por nome/caminho, filtro de extensão, ordenação, paginação e propriedades;
- importação dos dois esquemas CSV reconhecidos da amostra;
- tela de consulta de divergências importadas, com origem e linha do CSV;
- reimportação CSV transacional com savepoint, limpeza de linhas excedentes e reconciliação quando o esquema da fonte muda;
- núcleo Python inicial e testes do scanner, ainda não integrado ao desktop como sidecar.

**Ainda não é uma versão pronta para uso ou distribuição.** O build Windows também recebeu uma correção para gerar o ícone ICO exigido pelo Tauri; a correção aguarda confirmação no CI. A varredura é executada fora da thread da interface e informa progresso periodicamente; ainda não há cancelamento. Não há build Windows aprovado nem instalador validado. A leitura direta de DWG/PDF/Office, visualizadores, armazenamento binário de versões antigas, busca FTS5, exportações completas e auditoria operacional ainda precisam ser implementados.

## Próximas entregas

1. Estabilizar varredura, erros, reconciliação e build Windows.
2. Implementar versões imutáveis, migrações e backup SQLite.
3. Consolidar catálogo de projetos e busca FTS5.
4. Implementar leitura de PDF/Office e visualização segura.
5. Fazer prova de conceito DWG/DXF/IFC com validação de licença e compatibilidade.
6. Implementar comparação, recomendações explicáveis e exportação com evidências.
