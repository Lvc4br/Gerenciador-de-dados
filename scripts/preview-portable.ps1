# Executa a prévia visual do GED sem instalação administrativa.
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$desktopPath = Join-Path $repoRoot "apps\desktop"
$portableRoot = Join-Path $env:LOCALAPPDATA "GED-Portable"

New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null

Write-Host "GED - Preparando ambiente portátil..." -ForegroundColor Cyan

$nodeVersions = Invoke-RestMethod "https://nodejs.org/dist/index.json"
$release = $nodeVersions |
    Where-Object { $_.lts -and $_.files -contains "win-x64-zip" } |
    Select-Object -First 1

if (-not $release) {
    throw "Não foi possível localizar uma versão LTS portátil do Node.js."
}

$nodeVersion = $release.version
$nodeFolder = Join-Path $portableRoot "node-$nodeVersion-win-x64"
$zipPath = Join-Path $portableRoot "node-$nodeVersion-win-x64.zip"

if (-not (Test-Path (Join-Path $nodeFolder "node.exe"))) {
    Write-Host "Baixando Node.js $nodeVersion (portátil)..."
    $downloadUrl = "https://nodejs.org/dist/$nodeVersion/node-$nodeVersion-win-x64.zip"
    Invoke-WebRequest $downloadUrl -OutFile $zipPath
    Expand-Archive -Path $zipPath -DestinationPath $portableRoot -Force
}

$env:Path = "$nodeFolder;$env:Path"
$npm = Join-Path $nodeFolder "npm.cmd"

Write-Host "Node: $(& (Join-Path $nodeFolder 'node.exe') -v)"
Write-Host "NPM: $(& $npm -v)"

Set-Location $desktopPath

if (-not (Test-Path "node_modules")) {
    Write-Host "Instalando dependências do GED (primeira execução)..."
    & $npm install
    if ($LASTEXITCODE -ne 0) {
        throw "Falha ao instalar as dependências do GED."
    }
} else {
    Write-Host "Dependências já presentes; pulando reinstalação."
}

Write-Host ""
Write-Host "Prévia do GED: http://127.0.0.1:1420" -ForegroundColor Green
Write-Host "Mantenha esta janela aberta enquanto usar a prévia."
& $npm run preview
if ($LASTEXITCODE -ne 0) {
    throw "A prévia do GED terminou com erro."
}
