# Catálogo de projetos — especificação de implementação

## Objetivo

Separar o conceito de **projeto de engenharia** das pastas físicas indexadas. Uma pasta é uma origem de arquivos; um projeto é a unidade lógica usada para classificar documentos, disciplinas, revisões e divergências.

O catálogo deve funcionar offline, em SQLite, sem exigir servidor ou permissões administrativas.

## Modelo de dados proposto

### `projects`

| Campo | Tipo | Regra |
|---|---|---|
| id | INTEGER | PK autoincremental |
| code | TEXT | Obrigatório; único sem diferenciar maiúsculas/minúsculas |
| name | TEXT | Obrigatório |
| client | TEXT | Opcional |
| description | TEXT | Opcional |
| status | TEXT | `active` ou `archived`; padrão `active` |
| created_at | TEXT | UTC ISO-8601 |
| updated_at | TEXT | UTC ISO-8601 |

### `project_roots`

Relaciona projetos às raízes de biblioteca já cadastradas pelo indexador.

| Campo | Tipo | Regra |
|---|---|---|
| project_id | INTEGER | FK para projects, ON DELETE CASCADE |
| root_id | INTEGER | FK para a tabela de raízes existente |
| linked_at | TEXT | UTC ISO-8601 |

Chave primária composta: `(project_id, root_id)`. Uma raiz poderá ser associada a mais de um projeto, pois bibliotecas compartilhadas existem em engenharia.

### `disciplines`

Cadastro controlado por projeto: código, nome e ordem de exibição. A primeira carga pode sugerir Estruturas, Drenagem, Geometria, Pavimentação, Sinalização e Meio Ambiente, mas o usuário poderá criar outras disciplinas. Não presumir que todos os projetos usam a mesma estrutura.

### Vínculo de documentos

A associação entre arquivo e projeto/disciplina deve ser uma tabela própria, com origem do vínculo (`manual` ou `rule`), confiança e data. Não inferir vínculo apenas pelo nome do arquivo sem guardar evidência. Arquivos compartilhados podem ter mais de um vínculo lógico.

## Regras de integridade

1. Arquivos originais nunca são movidos, renomeados ou alterados pelo catálogo.
2. Arquivar um projeto não remove arquivos, raízes, hashes ou histórico.
3. Excluir um projeto deve exigir confirmação e remover apenas seus vínculos lógicos.
4. Código de projeto é único após normalização com `trim` e comparação sem distinção de caixa.
5. Caminhos físicos continuam pertencendo ao indexador; o catálogo guarda referências por ID.
6. Toda alteração de projeto deve atualizar `updated_at`.
7. Migrações devem ser transacionais e compatíveis com bancos existentes.

## Comandos de backend previstos

- `list_projects`: pesquisa por código/nome/cliente, filtro de status e paginação.
- `get_project`: detalhes, raízes e disciplinas.
- `create_project`: valida código e nome; retorna o registro criado.
- `update_project`: atualiza campos permitidos.
- `archive_project`: arquivamento reversível.
- `link_project_root` / `unlink_project_root`: associação sem alterar a pasta física.
- `list_project_files`: arquivos indexados nas raízes vinculadas, com paginação.
- `list_project_disciplines` / `save_project_discipline`: manutenção das disciplinas.

Todas as entradas devem ser validadas no Rust; a interface não é fronteira de segurança.

## Interface prevista

A seção **Projetos** terá:
- Lista com código, nome, cliente, status e quantidade de documentos.
- Busca e filtro Ativos/Arquivados.
- Formulário compacto de cadastro/edição.
- Detalhe com resumo, pastas vinculadas, disciplinas e documentos.
- Ação explícita para vincular uma pasta já indexada; não duplicar a indexação por padrão.

## Entrega incremental

1. Migração SQLite e testes de integridade/normalização.
2. Comandos Rust e testes unitários.
3. Interface de listagem e cadastro.
4. Vínculo de raízes e visualização dos arquivos do projeto.
5. Associação assistida de metadados, disciplinas e divergências, sempre com revisão humana.

## Critérios de aceite

- Criar dois projetos com códigos distintos e rejeitar duplicidade sem diferenciar caixa.
- Reiniciar o aplicativo e manter o catálogo.
- Vincular uma raiz a dois projetos sem duplicar registros de arquivos.
- Arquivar e reativar um projeto sem perda de dados.
- Impedir que excluir/arquivar um projeto altere os arquivos originais.
- Pesquisar por código e nome sem travar a interface em bibliotecas grandes.
- Abrir banco antigo e aplicar migração sem perda de dados.


## Estado da implementação (MVP)

A primeira entrega foi implementada na branch `feature/mvp-core-indexer`:

- Migração SQLite versão 4: `projects`, `project_roots` e `project_disciplines`.
- Backend Rust: listar, criar, atualizar, arquivar/reativar, listar raízes e vincular/desvincular raízes.
- Interface: lista pesquisável, filtro por situação, cadastro, detalhes, associação de pastas e arquivamento.
- Prévia de navegador: dados demonstrativos mantidos em memória; não substitui o teste com SQLite real.
- Testes Rust: normalização/duplicidade de código, compartilhamento de raiz e arquivamento.

### Limitações conhecidas desta entrega

- A interface ainda não oferece edição dos dados já cadastrados, embora o comando de backend exista.
- A tabela de disciplinas foi criada, mas o cadastro e a edição de disciplinas ainda não estão expostos na interface.
- O vínculo de documentos individuais ao projeto/disciplina ainda não foi implementado.
- A prévia visual não persiste dados após atualizar a página; a persistência real ocorre no aplicativo Tauri.

A geração do instalador Windows e os testes completos de CI precisam terminar antes de considerar esta versão validada para distribuição.


## Evolução — relações entre documentos e disciplinas (27/11/2026)

Foi adicionada a migração SQLite versão 5, sem alterar arquivos originais:

- `project_file_links`: vínculo lógico de um documento indexado a um projeto, com disciplina opcional, origem manual/regra e confiança.
- Um mesmo arquivo pode ser relacionado a mais de um projeto.
- A exclusão de uma disciplina mantém o vínculo do documento e define sua disciplina como nula (`ON DELETE SET NULL`).
- Comandos Rust adicionados: `list_project_disciplines`, `save_project_discipline`, `delete_project_discipline`, `list_project_files` e `set_project_file_link`.
- A listagem de documentos do projeto reúne arquivos das raízes vinculadas e documentos associados individualmente, com pesquisa e filtro de disciplina.

Esta entrega amplia o backend, mas ainda não significa que os novos recursos estejam expostos na interface. A tela de projetos precisa receber a próxima integração visual; a prévia de navegador também precisa de mocks correspondentes. Testes automatizados e CI devem validar a migração em bancos novos e existentes antes de uso com dados importantes.
