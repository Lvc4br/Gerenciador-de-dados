# Auditoria técnica e recomendação de stack — GED Desktop

**Data da revisão:** 27/11/2026  
**Escopo:** revisão estática do código e da documentação disponíveis na branch `feature/mvp-core-indexer`.  
**Plataforma definida:** Windows 10 ou superior, x64.  
**Uso inicial:** um engenheiro, um computador, operação local/offline.

> Esta é uma auditoria estática do estado do repositório. Não equivale a teste funcional em Windows, teste de carga, análise de segurança independente ou validação dos formatos CAD/BIM. A execução de CI consultada aprovou os testes Python e o build frontend, mas falhou no build nativo porque `icons/icon.ico` não existia. O `build.rs` foi corrigido para gerar o ícone; essa correção ainda aguarda uma nova confirmação de CI.

## 1. Parecer executivo

A direção tecnológica é adequada para um executável desktop Windows: Tauri 2 + React + TypeScript + Rust, com SQLite local. Python pode ser mantido para extração documental, OCR, classificação e adaptadores que tenham ecossistema melhor nessa linguagem.

O principal ajuste arquitetural recomendado é separar claramente as responsabilidades:
- **Rust/Tauri:** ciclo de vida desktop, permissões, acesso ao sistema de arquivos, comandos locais e operações rápidas.
- **React/TypeScript:** interface, navegação, tabelas, filtros, formulários e visualizadores web.
- **Python sidecar:** extração de conteúdo, OCR, parsing de formatos e regras de análise mais complexas, executados em jobs isolados.
- **SQLite:** catálogo local, metadados, estados dos jobs, auditoria e pesquisa textual FTS5.
- **Arquivos originais:** permanecem intactos nos caminhos escolhidos pelo usuário; previews e caches ficam em diretório próprio do GED.

**Não recomendo** adicionar agora PostgreSQL, Redis, servidor web, microsserviços ou nuvem. Isso aumentaria custo operacional sem benefício para o cenário single-user confirmado.

## 2. Estado observado no código

### Implementado na branch
- Shell desktop Tauri 2 com interface React/TypeScript.
- Configuração de instaladores NSIS e MSI no Tauri.
- Banco SQLite local com WAL e foreign keys.
- Seleção de pasta e varredura recursiva.
- Hash SHA-256 calculado em blocos.
- Registro de caminho relativo, nome, extensão, tamanho e data de modificação.
- Detecção de grupos de hashes iguais.
- Explorador com pesquisa por nome/caminho, filtro por extensão, ordenação e paginação.
- Importação de dois esquemas CSV reconhecidos: metadados de desenhos e relatório de divergências.
- Tela para consultar divergências importadas, preservando arquivo CSV de origem e número da linha.
- Núcleo Python separado com scanner e testes iniciais.

### Ainda não comprovado ou não implementado
- Não foi comprovado build bem-sucedido em Windows 10/11.
- Não há instalador executável validado para distribuição.
- O núcleo Python não está conectado como sidecar ao aplicativo Tauri; a varredura atual da aplicação é feita em Rust.
- Não há leitura direta do conteúdo DWG, PDF ou Office no fluxo do desktop.
- Não há visualizador de documentos integrado.
- Há histórico de fingerprints SHA-256 distintos por arquivo, com tamanho e datas; ainda não há cópias binárias para restauração.
- Não há busca textual FTS5, apesar de prevista na arquitetura.
- Não há módulo funcional de cadastro de projetos, exportações ou configurações; partes da navegação ainda são placeholders.
- Não há trilha de auditoria operacional completa.
- Os testes Python cobrem o scanner, mas não cobrem o fluxo Tauri/Rust, importação CSV, banco SQLite desktop ou interface.

## 3. Correções iniciadas após a auditoria

Na continuação da Fase 0, foram adicionadas à branch:
- Registro de execuções de varredura em `scan_runs`.
- Histórico de versões por SHA-256, com tamanho e datas, exibido nas propriedades do arquivo. O histórico registra fingerprints e metadados, não cópias físicas do conteúdo.
- Backup consistente do SQLite pela API online backup do SQLite, com verificação `integrity_check`.
- Restauração de backup GED validado, precedida por cópia automática do banco atual; falha na validação tenta recuperar o banco anterior.
- Teste unitário Rust para confirmar cópia do schema e dos registros por SQLite online backup.
- Migração compatível para adicionar `last_seen_scan_id` em bancos já existentes.
- Reconciliação de arquivos não encontrados: quando a travessia da pasta termina sem erro, os registros não vistos são marcados como `missing`, sem apagar seus dados.
- Erros de travessia impedem a marcação global de ausentes, evitando falsos positivos em varreduras parciais.
- Estados de arquivo `indexed`, `skipped`, `error` e `missing` expostos no Explorador.
- Varredura movida para `spawn_blocking`, evitando executar o hash pesado na thread de UI, com eventos periódicos de progresso para a interface.
- Contadores do dashboard e grupos de duplicidade consideram somente arquivos atualmente indexados.
- Reimportação CSV remove linhas antigas excedentes, limpa os dados do tipo anterior quando o esquema da fonte muda e usa savepoint para rollback integral em falhas.

**Limitações ainda abertas:** ainda não há cancelamento da varredura; o histórico atual não armazena cópias binárias, então não permite restaurar versões individuais de documentos. Backup e restauração do banco foram implementados; há teste unitário do mecanismo de cópia, mas ainda faltam teste funcional de ponta a ponta em Windows e teste com bancos maiores. As alterações aguardam confirmação do CI Windows.

## 4. Achados da auditoria — prioridade

| Prioridade | Achado | Risco / efeito | Recomendação |
|---|---|---|---|
| **Crítica** | Build e execução em Windows ainda não comprovados | O produto pode não compilar, instalar ou abrir no ambiente-alvo | Fazer primeiro build limpo em Windows 10 e 11 x64; guardar logs e artefatos de CI |
| **Alta — parcial** | Scanner agora usa `spawn_blocking` e informa progresso, mas ainda não pode ser cancelado | Uma varredura longa não pode ser interrompida pelo usuário | Implementar cancelamento cooperativo, com fechamento seguro da transação e atualização do estado da execução |
| **Resolvido na branch** | Arquivos removidos da pasta não eram reconciliados | O catálogo podia manter caminhos antigos como se estivessem presentes | `scan_runs` e `last_seen_scan_id` marcam itens como `missing` somente após travessia completa |
| **Parcial** | Erros de travessia são contabilizados e impedem reconciliação global, mas não há relatório detalhado por caminho | A equipe sabe que a varredura foi parcial, mas não identifica facilmente a pasta problemática | Registrar caminho, tipo e mensagem segura de cada falha |
| **Alta** | O mesmo scanner existe em Rust e Python, mas só o Rust está ligado ao app | Duas implementações podem divergir e gerar resultados diferentes | Definir um único scanner oficial. Recomendação: Rust para indexação local; Python apenas para parsing/análise |
| **Alta — parcial** | Reindexação atualiza os metadados atuais; histórico guarda SHA-256, tamanho e datas, mas não o binário | É possível provar que o conteúdo mudou, mas não restaurar uma versão antiga | Adicionar armazenamento de conteúdo versionado ou snapshot com política de retenção e backup |
| **Resolvido na branch** | Reimportação CSV podia manter linhas antigas e dados de esquema anterior | A tela podia exibir ocorrências obsoletas | Savepoint, remoção de linhas excedentes e limpeza ao mudar o tipo de esquema |
| **Média** | Busca do Explorer usa LIKE; FTS5 não foi criado | Busca em grandes catálogos ficará limitada e menos relevante | Criar índice FTS5 para nome, título, identificadores e texto extraído; manter filtros estruturados em SQL |
| **Média** | Erros de importação CSV são somados ao contador genérico de indexação | O usuário não sabe se falhou o hash, a leitura ou a interpretação do CSV | Separar erros de acesso, hash, formato e extração; exibir relatório por arquivo |
| **Média** | Interface contém ações e módulos ainda sem comportamento | Pode induzir o usuário a esperar funcionalidades inexistentes | Ocultar/desabilitar ações não entregues e mostrar claramente “em desenvolvimento” |
| **Média — parcial** | Migrações SQLite ainda não estão versionadas; backup/restauração foram adicionados | Mudanças futuras de schema podem ficar frágeis, embora exista recuperação do banco | Adotar tabela `schema_migrations` e validar rotina de backup/restauração em cenários reais |
| **Média** | Testes automatizados não cobrem Rust/CSV/integração/UI | Regressões podem chegar ao usuário | Criar testes unitários Rust, fixtures CSV reais anonimizadas, testes de migração e smoke test do instalador |
| **Média** | Dependências e formatos CAD/BIM ainda não passaram por validação de licença/versão | Risco de custo, incompatibilidade ou restrição de distribuição | Fazer prova de conceito isolada para cada formato e registrar matriz de compatibilidade/licenças |

## 5. Recomendação de linguagens e tecnologias

| Camada | Recomendação | Motivo |
|---|---|---|
| Aplicativo desktop | **Tauri 2 + Rust** | Binário nativo, integração com Windows e permissões explícitas; adequado para empacotar uma aplicação desktop |
| Interface | **React + TypeScript** | Componentização, tipagem, ecossistema de tabelas, formulários e visualizadores |
| Estilo | CSS modular ou Tailwind (escolher um padrão) | Evitar misturar estratégias de estilo sem necessidade |
| Banco local | **SQLite + FTS5** | Sem servidor, simples de distribuir, transações e busca textual local |
| Extração documental | **Python 3.12+** | Ecossistema amplo de OCR, PDF, Office, dados e processamento de documentos |
| Empacotamento Python | **PyInstaller**, se o sidecar for necessário | Permite distribuir o motor sem exigir Python instalado; o build Windows deve ser feito em Windows |
| PDF | PDF.js | Renderização e seleção de texto no frontend; validar documentos grandes e PDFs digitalizados |
| OCR | Tesseract como opção local inicial; avaliar PaddleOCR apenas se a qualidade justificar | OCR offline; comparar precisão em carimbos, desenhos e tabelas reais |
| CSV/XLSX/DOCX/PPTX | bibliotecas Python especializadas por formato | Extração controlada, com testes por versão e amostras reais |
| DWG/DXF | prova de conceito com SDK/biblioteca compatível e licença validada | DWG é o maior risco de compatibilidade/licenciamento; não escolher biblioteca só por ser gratuita |
| IFC/BIM | IfcOpenShell | Especialização em IFC; validar versões IFC e campos necessários |
| Testes | pytest + testes Rust + Playwright para fluxos de UI, quando aplicável | Cobertura das três camadas |
| CI/CD | GitHub Actions em Windows x64 | Reproduzir o ambiente real de build e gerar instaladores de teste |
| Logs | tracing no Rust e logging estruturado no Python | Diagnóstico sem registrar conteúdo sensível dos documentos |

### Decisão sobre Python

Não mover tudo para Python. O Rust já está integrado ao Tauri e é adequado para operações de sistema. Python deve entrar quando houver uma tarefa de extração ou análise que justifique seu ecossistema. O sidecar precisa ter protocolo IPC local restrito, versão/health check, timeout, cancelamento e limites de memória; não deve abrir uma API de rede acessível na LAN.

## 6. Modelo de dados que recomendo consolidar

Separar os conceitos abaixo:
1. **Raiz de biblioteca:** pasta que o usuário autorizou o GED a catalogar.
2. **Projeto:** obra/empreendimento lógico, com código, nome, disciplina, local, etapa e atributos.
3. **Documento:** identidade lógica do desenho/relatório, independente do caminho físico.
4. **Versão do documento:** arquivo físico específico, com hash SHA-256, tamanho, caminho, data e revisão.
5. **Extração:** resultado de um parser específico, com versão do extrator, estado, data e erro.
6. **Valor de metadado:** valor bruto e normalizado, origem, localização e confiança.
7. **Achado:** divergência ou alerta gerado por regra versionada, com evidências e estado de revisão.
8. **Evento de auditoria:** quem fez o quê, quando, em qual entidade e com qual justificativa.

Não tratar hash igual como sinônimo de mesmo projeto. Hash igual significa conteúdo binário idêntico; semelhança de projeto deve usar atributos e evidências distintas.

## 7. Regras de qualidade e segurança

- Nunca modificar originais automaticamente.
- Não executar macros, scripts ou conteúdo ativo de documentos.
- Processar formatos complexos em subprocessos com timeout e limite de memória.
- Preservar valor original extraído e valor normalizado lado a lado.
- Guardar origem e localização de cada metadado: nome do arquivo, página, folha, bloco, célula ou entidade.
- Exibir confiança apenas quando houver calibração; não inventar percentuais.
- Toda divergência automática deve ser um alerta revisável, nunca uma reprovação técnica automática.
- Toda decisão humana (confirmar, rejeitar, resolver, aceitar risco) deve deixar registro.
- Criar backup do catálogo e testar restauração, não apenas criação.
- Não enviar documentos a serviços externos sem consentimento explícito.
- Assinar o instalador antes de distribuição ampla e verificar atualizações de forma segura.

## 8. Plano de execução recomendado

### Fase 0 — estabilização técnica
- Validar build e execução no Windows 10/11 x64.
- Corrigir reconciliação de arquivos ausentes e erros de varredura.
- Tornar indexação assíncrona com progresso/cancelamento.
- Definir scanner único.
- Criar migrações SQLite versionadas; backup/restauração já implementados, faltando validação funcional e teste com bases maiores.
- Corrigir importação CSV para substituir o conjunto de linhas da fonte sem deixar registros órfãos.

**Critério de aceite:** pasta de teste indexada sem travar a interface; erros reportados; reindexação idempotente; remoções detectadas; banco recuperável por backup.

### Fase 1 — catálogo confiável
- Cadastro de projetos e associação de documentos.
- Modelo de versões imutáveis.
- FTS5 e filtros por metadados.
- Histórico e trilha de auditoria.
- Importação CSV com relatório de qualidade.

**Critério de aceite:** pesquisar por código, título, ramo, rodovia, trecho e revisão; localizar a origem de cada dado; preservar versões anteriores.

### Fase 2 — leitura documental
- PDF pesquisável e PDF digitalizado com OCR.
- Extração de propriedades e texto de Office.
- Preview seguro e cache local.
- Fila de processamento e reprocessamento.

**Critério de aceite:** cada campo extraído informa origem, método, versão do extrator e possibilidade de revisão.

### Fase 3 — CAD/BIM e comparação
- Prova de conceito DWG/DXF/IFC com arquivos reais.
- Extração de carimbo e atributos CAD.
- Comparação de versões e regras de divergência.
- Matriz de compatibilidade por versão de formato.

**Critério de aceite:** relatório de comparação com evidências rastreáveis e validação por engenheiro.

### Fase 4 — recomendação e exportação
- Similaridade por atributos técnicos e textuais.
- Explicação do motivo de cada recomendação.
- Exportação XLSX/PDF/CSV com filtros e evidências.
- Pacote de projeto com manifesto e hashes.

**Critério de aceite:** recomendação explicável; nenhuma indicação automática de adequação técnica sem aprovação humana.

### Fase 5 — distribuição
- Instalador NSIS/MSI validado.
- Assinatura digital.
- Teste em máquina limpa Windows 10 e Windows 11.
- Backup, restauração, atualização e desinstalação.
- Manual de instalação e operação.

## 9. Indicadores mínimos de aceitação

- 0 alteração não autorizada em arquivos originais.
- 100% dos arquivos processados com status explícito: indexado, ignorado, ausente ou erro.
- 100% dos metadados extraídos com proveniência.
- 100% dos achados com evidência e regra identificável.
- Reindexação repetida sem duplicar linhas de metadados.
- Backup restaurado com sucesso em teste.
- Testes de instalação e inicialização em Windows 10/11 x64.
- Nenhuma dependência de servidor ou internet para as funções locais essenciais.

## 10. Referências técnicas oficiais

- Tauri 2 — arquitetura: https://v2.tauri.app/concept/architecture/
- Tauri 2 — início e distribuição: https://v2.tauri.app/start/
- SQLite FTS5: https://www.sqlite.org/fts5.html
- SQLite WAL: https://sqlite.org/wal.html
- PyInstaller: https://pyinstaller.org/en/latest/
- IfcOpenShell: https://docs.ifcopenshell.org/
- PDF.js: https://mozilla.github.io/pdf.js/

## 11. Conclusão

A stack atual deve ser mantida, com uma decisão arquitetural clara: **Rust é o núcleo desktop e de indexação local; Python é o motor de extração/análise; React/TypeScript é a interface; SQLite é o banco embarcado**.

O maior risco neste momento não é escolher outra linguagem. É ampliar funcionalidades antes de estabilizar indexação, versões, proveniência, recuperação de dados, testes Windows e compatibilidade/licenças CAD. A próxima entrega deve priorizar a Fase 0, antes de começar OCR ou comparação DWG.


## Atualização de implementação — 27/11/2026

- O banco SQLite agora registra as migrações aplicadas em `schema_migrations`, com baseline para bancos existentes e uma segunda migração de índices voltados à consulta do catálogo.
- A migração de índices é transacional e idempotente; a abertura do GED continua compatível com bancos criados por versões anteriores do MVP.
- A criação de backup agora verifica também as relações por `PRAGMA foreign_key_check`; a cópia de segurança pré-restauração usa timestamp em milissegundos para reduzir colisões de nome.
- FTS5 ainda não está implementado. Antes de ativá-lo, será necessário validar a compilação SQLite usada no instalador Windows e medir tamanho do índice, tempo de indexação e desempenho em hardware modesto.
- Não há ainda teste funcional de migração com banco legado real nem execução confirmada do CI para este commit.
