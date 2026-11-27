# GED — Gerenciador de Engenharia e Documentos

Plataforma para organizar, localizar, analisar, comparar e reutilizar projetos anteriores de engenharia, com foco inicial em engenharia civil e documentação técnica.

> **Objetivo:** transformar arquivos históricos dispersos em uma base documental pesquisável, rastreável e confiável para apoiar engenheiros na localização de referências e na análise de documentos.

## Visão do produto

O GED não é apenas um gerenciador de arquivos. É um sistema de inteligência documental que combina:
- Explorador de arquivos familiar, com navegação em árvore, breadcrumbs, visualização rápida e painel de propriedades.
- Importação em lote e indexação de documentos heterogêneos.
- Extração de metadados de arquivos, carimbos, textos, tabelas e propriedades CAD/BIM.
- Classificação automática assistida e taxonomia configurável.
- Busca textual, filtros facetados, busca por similaridade e recomendações explicáveis.
- Detecção de divergências, duplicidades, lacunas e conflitos entre metadados e documentos relacionados.
- Comparação de revisões e emissão de relatórios.
- Exportação de arquivos originais, metadados, resultados de pesquisa e pacotes documentais.
- Controle de acesso, auditoria, preservação dos originais e rastreabilidade.

## Evidência inicial: conjunto AB20A

Foi inspecionado o pacote de amostra `AB20A.zip`, com:
- 3 desenhos DWG;
- `titulos_carimbo.csv`: 1.336 linhas e 14 colunas;
- `divergencias_numero_carimbo.csv`: 9 linhas e 11 colunas.

Os CSVs usam delimitador ponto e vírgula e codificação UTF-8 com BOM. Entre os campos encontrados estão arquivo, ramo, candidato a título, rodovia, trecho, escala, desenho ARTESP, desenho interno, número no nome, número no carimbo, revisão, diagnóstico e caminho original.

A amostra demonstra um caso de uso concreto: conferir se o identificador do arquivo, o número no carimbo e o título associado ao desenho são coerentes. Os CSVs são evidência de metadados extraídos; não substituem a inspeção integral dos DWGs nem uma validação técnica do projeto.

## Documentação

- [Arquitetura](docs/ARCHITECTURE.md)
- [Modelo de dados e schema SQL](docs/DATA_MODEL.md)
- [Experiência e layout da interface](docs/UX.md)
- [Plano de desenvolvimento e critérios de aceite](docs/ROADMAP.md)
- [Matriz inicial de formatos](docs/FORMAT_SUPPORT.md)

## Stack recomendada

| Camada | Tecnologia |
|---|---|
| Frontend | React + TypeScript |
| UI | Tailwind CSS + biblioteca acessível de componentes |
| Backend | Python + FastAPI |
| Banco transacional | PostgreSQL |
| Busca textual/facetas | OpenSearch (fase de escala; PostgreSQL no MVP) |
| Busca vetorial | pgvector |
| Arquivos | S3 compatível / MinIO |
| Filas | Celery + Redis |
| Visualização | PDF.js; visualizadores CAD/BIM por adaptadores |
| Infraestrutura | Docker Compose no desenvolvimento; implantação conteinerizada |
| Testes | Pytest, testes de contrato e Playwright |

## Princípios não negociáveis

1. Nunca alterar o arquivo original durante ingestão, leitura ou análise.
2. Calcular hash SHA-256 e preservar proveniência, datas e origem.
3. Não confundir alerta automatizado com não conformidade de engenharia.
4. Mostrar evidências, regra aplicada, confiança e versão do analisador em cada achado.
5. Não sobrescrever revisões: cada versão é imutável e ligada à anterior quando possível.
6. Permitir correção humana, registrar quem corrigiu e preservar o valor extraído.
7. Respeitar licenças de bibliotecas, conversores e formatos proprietários.
8. Aplicar autorização no backend, não apenas esconder ações na interface.
9. Exportar sempre com manifesto, origem e trilha de auditoria quando aplicável.
10. Tratar arquivos não suportados ou corrompidos como falhas explícitas, nunca como sucesso silencioso.

## Estado

Este repositório contém a especificação inicial e a arquitetura de referência. O produto ainda precisa ser implementado e validado com usuários, arquivos reais e regras de engenharia da organização.

## Próximo passo

Implementar o MVP vertical: autenticação, cadastro de projeto, upload em lote, armazenamento imutável, extração de metadados CSV/PDF, explorador, busca e relatório de divergências.
