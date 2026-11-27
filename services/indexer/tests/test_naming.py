from ged_indexer.naming import extract_filename_metadata


def test_extracts_project_discipline_number_and_revision():
    result = extract_filename_metadata("AB20A-EST-001_REV02.dwg")
    assert result == {
        "project_prefix": "AB20A",
        "discipline_code": "EST",
        "drawing_number": "EST-001",
        "revision": "02",
        "confidence": 0.92,
        "source": "filename",
    }


def test_extracts_number_without_revision_as_suggestion():
    result = extract_filename_metadata("AB20A-DRE-014.dwg")
    assert result["project_prefix"] == "AB20A"
    assert result["discipline_code"] == "DRE"
    assert result["drawing_number"] == "DRE-014"
    assert result["revision"] is None
    assert result["confidence"] == 0.78


def test_does_not_invent_metadata_for_unstructured_filename():
    result = extract_filename_metadata("Memorial_Descritivo_final.pdf")
    assert result["project_prefix"] is None
    assert result["discipline_code"] is None
    assert result["drawing_number"] is None
    assert result["revision"] is None
    assert result["confidence"] == 0.0


def test_detects_revision_without_drawing_number_as_low_confidence():
    result = extract_filename_metadata("Memorial_REV03.pdf")
    assert result["revision"] == "03"
    assert result["drawing_number"] is None
    assert result["confidence"] == 0.45
