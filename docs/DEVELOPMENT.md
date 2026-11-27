# Desenvolvimento do GED Desktop

## Plataforma-alvo

Windows 10 ou superior, 64 bits. O build de distribuição deve ser executado e validado em Windows x64.

## Ferramentas necessárias

- Node.js LTS e npm.
- Rust stable com toolchain MSVC.
- Microsoft C++ Build Tools / Visual Studio Build Tools.
- WebView2 Runtime (normalmente já presente em versões atuais do Windows).
- Python 3.10+ apenas para desenvolver e testar os parsers do serviço Python.

## Abrir uma prévia visual no VS Code (sem Rust)

Esta opção permite conhecer a interface antes de compilar o aplicativo desktop. Ela usa dados demonstrativos e não acessa pastas, SQLite, backups ou o indexador real.

1. Abra a pasta raiz do repositório no VS Code.
2. Abra **Terminal → Novo Terminal**.
3. Execute:

```powershell
cd apps/desktop
npm install
npm run preview
```

4. Abra no navegador o endereço indicado pelo Vite (por padrão, `http://localhost:1420`).

Na prévia, é possível navegar pela Visão geral, Explorador e Divergências, testar pesquisa, filtros, ordenação e paginação. Os botões de importação, backup e restauração explicam que essas operações exigem o aplicativo desktop. Os dados exibidos são fictícios e não devem ser usados como registros de engenharia.

## Executar o aplicativo desktop real em desenvolvimento

Para testar o backend Rust, SQLite, diálogos nativos e indexação de pastas, instale também Rust stable/MSVC, C++ Build Tools e WebView2 Runtime. No terminal, entre em `apps/desktop`:

```powershell
npm install
npm run tauri dev
```

O aplicativo abre em uma janela nativa. A ação **Importar pasta** abre o seletor de diretório do Windows e inicia a indexação local.

## Gerar instalador

Ainda em `apps/desktop`:

```powershell
npm run tauri build
```

Os instaladores são gerados em `src-tauri/target/release/bundle`. A configuração atual solicita os formatos NSIS e MSI.

## Testar o núcleo Python

No diretório `services/indexer`:

```powershell
py -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -e .[test]
pytest
```

## Banco de dados local

O aplicativo cria `ged.sqlite3` no diretório de dados da aplicação do usuário, obtido pela API de caminhos do Tauri. O banco não fica dentro da pasta de instalação.

## Escopo desta primeira fatia

- Seleção de uma pasta pelo diálogo nativo.
- Percurso de diretórios sem seguir links simbólicos.
- Limite inicial de 2 GiB por arquivo.
- Hash SHA-256 para cada arquivo legível.
- Registro de caminho relativo, extensão, tamanho e data de modificação.
- Identificação de grupos de arquivos com conteúdo idêntico.
- Explorador conectado ao banco local com pesquisa por nome/caminho, filtro de extensão, ordenação e paginação.
- Painel de propriedades com caminho, tipo, tamanho, data de modificação e SHA-256.
- Preservação dos originais: não há operação de escrita, renomeação ou exclusão nos arquivos indexados.

A indexação reconhece os dois esquemas CSV de metadados de desenhos e divergências. Ela ainda não interpreta diretamente o conteúdo binário de DWG, PDF ou Office. O Explorador lista os arquivos indexados, mas ainda não abre nem renderiza seu conteúdo. Também não classifica automaticamente arquivos como projetos; registra pastas-raiz e arquivos. Esses recursos pertencem às próximas etapas.
