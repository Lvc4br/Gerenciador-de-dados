from __future__ import annotations

from pathlib import Path

MAX_PDF_BYTES = 200 * 1024 * 1024
MAX_PAGES = 5000
MAX_EXTRACTED_CHARACTERS = 10_000_000

class PdfExtractionError(RuntimeError):
    """Falha controlada durante a leitura de um PDF."""

def extract_pdf_text(path: str | Path) -> dict[str, int | str]:
    pdf_path = Path(path)
    if pdf_path.suffix.lower() != ".pdf":
        raise PdfExtractionError("O arquivo informado não possui extensão PDF.")
    try:
        stat = pdf_path.stat()
    except OSError as exc:
        raise PdfExtractionError(f"Não foi possível acessar o PDF: {exc}") from exc
    if not pdf_path.is_file():
        raise PdfExtractionError("O caminho informado não é um arquivo.")
    if stat.st_size > MAX_PDF_BYTES:
        raise PdfExtractionError("PDF excede o limite de 200 MB para extração.")
    try:
        from pypdf import PdfReader
    except ImportError as exc:
        raise PdfExtractionError(
            "Dependência ausente. Instale o extra: pip install -e '.[pdf]'"
        ) from exc
    try:
        reader = PdfReader(str(pdf_path), strict=False)
        if reader.is_encrypted:
            try:
                unlocked = reader.decrypt("")
            except Exception as exc:
                raise PdfExtractionError("PDF protegido por senha; não foi possível abrir.") from exc
            if not unlocked:
                raise PdfExtractionError("PDF protegido por senha; senha não fornecida.")
        total_pages = len(reader.pages)
        if total_pages > MAX_PAGES:
            raise PdfExtractionError(f"PDF excede o limite de {MAX_PAGES} páginas.")
        chunks: list[str] = []
        characters = 0
        pages_with_text = 0
        for page_number, page in enumerate(reader.pages, start=1):
            try:
                text = page.extract_text() or ""
            except Exception as exc:
                raise PdfExtractionError(f"Falha ao extrair a página {page_number}: {exc}") from exc
            if text.strip():
                pages_with_text += 1
                chunks.append(f"\n\n--- Página {page_number} ---\n{text}")
                characters += len(text)
            if characters > MAX_EXTRACTED_CHARACTERS:
                raise PdfExtractionError(
                    f"Extração excedeu o limite de {MAX_EXTRACTED_CHARACTERS} caracteres."
                )
        return {
            "text": "".join(chunks).strip(),
            "page_count": total_pages,
            "pages_with_text": pages_with_text,
            "character_count": characters,
            "parser": "pypdf",
        }
    except PdfExtractionError:
        raise
    except Exception as exc:
        raise PdfExtractionError(f"PDF inválido ou não suportado: {exc}") from exc
