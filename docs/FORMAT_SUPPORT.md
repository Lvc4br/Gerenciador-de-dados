# Matriz de formatos e política de conversão

O GED deve usar uma matriz explícita de capacidades. “Importar”, “visualizar”, “extrair metadados”, “comparar” e “exportar” são capacidades diferentes e não devem ser tratadas como sinônimos.

| Família | Exemplos | Importar/indexar | Preview | Exportar |
|---|---|---|---|---|
| Texto/dados | TXT, CSV, JSON, XML | Sim, parser seguro | Sim | CSV, JSON, TXT |
| PDF | PDF textual e digitalizado | Sim; OCR opcional | Sim, PDF.js | Original; relatório PDF |
| Imagens | PNG, JPG, TIFF | Sim | Sim | Original; derivados |
| Office | DOCX, XLSX, PPTX | Via parser/conversor isolado | Via preview | Original; CSV/PDF quando suportado |
| CAD | DWG, DXF, DGN | Depende do adaptador/versão/licença | Depende do visualizador | Original; conversão apenas validada |
| BIM | IFC, RVT | IFC aberto é prioridade; RVT depende de SDK/serviço | IFC.js ou solução compatível | IFC quando aplicável; original |
| Compactados | ZIP | Inspeção segura e extração controlada | Não aplicável | Original/pacote |

## Política

1. Preservar o arquivo original e seu hash.
2. Converter somente para preview ou exportação explicitamente solicitada.
3. Registrar ferramenta, versão, parâmetros, data e resultado da conversão.
4. Nunca afirmar que um PDF gerado é equivalente ao DWG/RVT original.
5. Bloquear path traversal, zip bombs, macros, arquivos executáveis e formatos com risco não tratado.
6. Definir limites configuráveis de tamanho, profundidade de compactação, tempo e memória.
7. Validar MIME real e assinatura binária; não confiar apenas na extensão.
8. Para formatos proprietários, verificar licença, redistribuição, uso em servidor e limites comerciais antes da escolha.

## Prioridade inicial

1. CSV/TXT/JSON.
2. PDF textual e digitalizado (OCR).
3. PNG/JPG/TIFF.
4. DOCX/XLSX/PPTX.
5. DWG/DXF por prova técnica com arquivos de versões reais.
6. IFC e formatos BIM adicionais.

## Observação sobre a amostra AB20A

A amostra contém três arquivos DWG e dois CSVs de metadados. A inspeção dos CSVs confirmou o delimitador ponto e vírgula e UTF-8 com BOM. O suporte completo aos desenhos depende de testar os DWGs reais com o adaptador e a versão de ferramenta escolhidos.
