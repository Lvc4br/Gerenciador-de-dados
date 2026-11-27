from __future__ import annotations

import sys
from types import SimpleNamespace

import pytest

from ged_indexer.pdf import PdfExtractionError, extract_pdf_text


class FakePage:
    def __init__(self, text: str):
        self.text = text

    def extract_text(self):
        return self.text


class FakeReader:
    def __init__(self, _path: str, strict: bool = False):
        self.is_encrypted = False
        self.pages = [FakePage("Número do desenho: EST-001"), FakePage("Revisão: 02"), FakePage("")]


def test_extracts_text_with_page_boundaries(tmp_path, monkeypatch):
    pdf = tmp_path / "desenho.pdf"
    pdf.write_bytes(b"%PDF-fake")
    monkeypatch.setitem(sys.modules, "pypdf", SimpleNamespace(PdfReader=FakeReader))

    result = extract_pdf_text(pdf)

    assert result["page_count"] == 3
    assert result["pages_with_text"] == 2
    assert result["character_count"] > 0
    assert "EST-001" in result["text"]
    assert "--- Página 2 ---" in result["text"]


def test_rejects_non_pdf(tmp_path):
    file = tmp_path / "documento.txt"
    file.write_text("texto", encoding="utf-8")
    with pytest.raises(PdfExtractionError, match="extensão PDF"):
        extract_pdf_text(file)


def test_rejects_pdf_above_size_limit(tmp_path, monkeypatch):
    import ged_indexer.pdf as pdf_module

    file = tmp_path / "grande.pdf"
    file.write_bytes(b"%PDF-fake")
    monkeypatch.setattr(pdf_module, "MAX_PDF_BYTES", 1)
    with pytest.raises(PdfExtractionError, match="200 MB"):
        extract_pdf_text(file)


def test_rejects_password_protected_pdf(tmp_path, monkeypatch):
    class EncryptedReader:
        is_encrypted = True
        pages = []

        def __init__(self, _path: str, strict: bool = False):
            pass

        def decrypt(self, _password: str):
            return 0

    file = tmp_path / "protegido.pdf"
    file.write_bytes(b"%PDF-fake")
    monkeypatch.setitem(sys.modules, "pypdf", SimpleNamespace(PdfReader=EncryptedReader))
    with pytest.raises(PdfExtractionError, match="protegido por senha"):
        extract_pdf_text(file)


def test_extracts_real_pdf_text_with_pypdf(tmp_path):
    from pypdf import PdfWriter
    from pypdf.generic import DecodedStreamObject, DictionaryObject, NameObject

    pdf = tmp_path / "real.pdf"
    writer = PdfWriter()
    page = writer.add_blank_page(width=300, height=300)
    font = DictionaryObject({
        NameObject("/Type"): NameObject("/Font"),
        NameObject("/Subtype"): NameObject("/Type1"),
        NameObject("/BaseFont"): NameObject("/Helvetica"),
    })
    page[NameObject("/Resources")] = DictionaryObject({
        NameObject("/Font"): DictionaryObject({NameObject("/F1"): font})
    })
    stream = DecodedStreamObject()
    stream.set_data(b"BT /F1 12 Tf 20 200 Td (EST-001 REV02) Tj ET")
    page[NameObject("/Contents")] = stream
    with pdf.open("wb") as output:
        writer.write(output)

    result = extract_pdf_text(pdf)

    assert result["page_count"] == 1
    assert result["pages_with_text"] == 1
    assert "EST-001 REV02" in result["text"]
