# GED — Gerenciador de Engenharia e Documentos

Aplicativo **desktop para Windows**, distribuído como instalador e executável `.exe`, para organizar, localizar, analisar, comparar e reutilizar projetos anteriores de engenharia civil.

> **Objetivo:** transformar arquivos históricos dispersos em uma base documental local, pesquisável, rastreável e confiável para apoiar engenheiros.

## Produto e plataforma

O GED é um programa instalado no computador do usuário. A operação principal deve funcionar **offline**, sem depender de navegador, servidor remoto ou conexão com a internet. Acesso a pastas locais, discos externos e compartilhamentos de rede será parte central do produto.

A arquitetura também deve permitir, futuramente, sincronização opcional com um servidor corporativo para equipes e múltiplos computadores. Essa evolução não pode ser requisito para o MVP.

## Visão do produto

- Explorador semelhante ao Windows Explorer, com árvore de pastas, breadcrumbs, lista/grade, preview e propriedades.
- Importação/indexação de pastas inteiras e discos, sem modificar os originais.
- Extração de metadados de arquivos, carimbos, textos, tabelas e propriedades CAD/BIM.
- Classificação assistida e taxonomia configurável.
- Busca textual, filtros, similaridade técnica e recomendações explicáveis.
- Detecção de divergências, duplicidades, lacunas e conflitos.
- Comparação de revisões e emissão de relatórios.
- Exportação de originais, metadados, resultados e pacotes.
- Banco de dados local, backup, auditoria e controle de acesso local.

## Stack desktop recomendada

| Camada | Tecnologia |
|---|---|
| Aplicativo Windows | Tauri 2 |
| Interface | React + TypeScript |
| Estilo e componentes | Tailwind CSS + componentes acessíveis |
| Núcleo de processamento | Python empacotado como sidecar (PyInstaller) |
| Comunicação UI/núcleo | Tauri commands + IPC local restrito |
| Banco local | SQLite + FTS5; migrações via SQLAlchemy/Alembic ou equivalente |
| Busca semântica | Índice vetorial local opcional, com fallback semântico/lexical |
| Arquivos | Sistema de arquivos local; originais preservados no local escolhido |
| Filas | SQLite job queue / worker local; Redis não é obrigatório no desktop |
| PDF e imagens | PDF.js e bibliotecas de imagem |
| CAD/BIM | Adaptadores isolados; validar SDK, versão e licença |
| Empacotamento | Tauri bundler + instalador Windows (.msi/.exe) |
| Testes | Pytest, Vitest e Playwright/Webdriver conforme camada |

**Por que Tauri:** entrega uma aplicação desktop leve, com interface web moderna e acesso controlado a recursos nativos. Python fica responsável pelo ecossistema de extração documental e engenharia. O sidecar será empacotado junto ao aplicativo; o usuário não precisará instalar Python.

## Evidência inicial: conjunto AB20A

Foi inspecionado o pacote de amostra `AB20A.zip`, com:
- 3 desenhos DWG;
- `titulos_carimbo.csv`: 1.336 linhas e 14 colunas;
- `divergencias_numero_carimbo.csv`: 9 linhas e 11 colunas.

Os CSVs usam delimitador ponto e vírgula e codificação UTF-8 com BOM. Entre os campos encontrados estão arquivo, ramo, candidato a título, rodovia, trecho, escala, desenho ARTESP, desenho interno, número no nome, número no carimbo, revisão, diagnóstico e caminho original.

A amostra demonstra um caso de uso concreto: conferir se o identificador do arquivo, o número no carimbo e o título associado ao desenho são coerentes. Os CSVs são evidência de metadados extraídos; não substituem a inspeção integral dos DWGs nem uma validação técnica do projeto.

## Documentação

- [Arquitetura desktop](docs/ARCHITECTURE.md)
- [Modelo de dados e schema](docs/DATA_MODEL.md)
- [Experiência e layout](docs/UX.md)
- [Plano de desenvolvimento](docs/ROADMAP.md)
- [Matriz de formatos](docs/FORMAT_SUPPORT.md)
- [Backend e comunicação local](docs/API_AND_BACKEND.md)

## Princípios não negociáveis

1. Nunca alterar o arquivo original durante ingestão, leitura ou análise.
2. Calcular hash SHA-256 e preservar proveniência, datas e origem.
3. Não confundir alerta automatizado com não conformidade de engenharia.
4. Mostrar evidências, regra aplicada, confiança e versão do analisador em cada achado.
5. Não sobrescrever revisões: cada versão é imutável e ligada à anterior quando possível.
6. Permitir correção humana, registrar quem corrigiu e preservar o valor extraído.
7. Respeitar licenças de bibliotecas, conversores e formatos proprietários.
8. Restringir acesso a arquivos locais aos diretórios autorizados pelo usuário.
9. Exportar com manifesto, origem e trilha de auditoria quando aplicável.
10. Tratar arquivos não suportados ou corrompidos como falhas explícitas, nunca como sucesso silencioso.
11. Operações essenciais devem funcionar offline.
12. Não enviar documentos para serviços externos/IA em nuvem sem consentimento explícito.

## Estado

Este repositório contém a especificação inicial. O aplicativo executável ainda precisa ser implementado, empacotado e validado em máquinas Windows e com arquivos reais.

## Próximo passo

Implementar o MVP vertical desktop: shell Tauri, interface Explorer, sidecar Python, banco SQLite, indexação de pastas, leitura CSV/PDF e relatório de divergências usando a amostra AB20A.
