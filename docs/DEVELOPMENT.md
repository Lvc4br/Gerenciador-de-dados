# Desenvolvimento do GED Desktop

## Plataforma-alvo

Windows 10 ou superior, 64 bits. O build de distribuição deve ser executado e validado em Windows x64.

## Ferramentas necessárias

- Node.js LTS e npm.
- Rust stable com toolchain MSVC.
- Microsoft C++ Build Tools / Visual Studio Build Tools.
- WebView2 Runtime (normalmente já presente em versões atuais do Windows).
- Python 3.10+ apenas para desenvolver e testar os parsers do serviço Python.

## Executar a interface desktop em desenvolvimento

No terminal, entre em `apps/desktop`:

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
