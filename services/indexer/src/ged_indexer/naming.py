from __future__ import annotations

import re
from pathlib import Path

# Conservative filename-only heuristics. Results are suggestions, never assignments.
_DRAWING_NUMBER = re.compile(
    r"(?:^|[-_.])(?P<discipline>[A-Z]{2,5})[-_]?(?P<number>\\d{2,5})(?=$|[-_.])"
)
_REVISION = re.compile(
    r"(?:^|[-_.])(?:REV(?:ISION|ISAO)?[-_ .]?|R[-_ .]?)(?P<revision>\\d{1,3}|[A-Z])(?=$|[-_.])"
)
_PROJECT_PREFIX = re.compile(r"^(?P<prefix>[A-Z]{1,6}\\d{1,4}[A-Z]?)\b")


def extract_filename_metadata(filename: str | Path) -> dict[str, str | float | None]:
    """Extract cautious drawing metadata from a filename without opening the file."""
    name = Path(filename).stem.upper()
    drawing_match = _DRAWING_NUMBER.search(name)
    revision_match = _REVISION.search(name)
    project_match = _PROJECT_PREFIX.match(name)

    discipline = drawing_match.group("discipline") if drawing_match else None
    number = drawing_match.group("number") if drawing_match else None
    revision = revision_match.group("revision") if revision_match else None

    # High confidence only when both a discipline-number pair and revision marker exist.
    if drawing_match and revision_match:
        confidence = 0.92
    elif drawing_match:
        confidence = 0.78
    elif revision_match:
        confidence = 0.45
    else:
        confidence = 0.0

    drawing_number = f"{discipline}-{number}" if discipline and number else None
    return {
        "project_prefix": project_match.group("prefix") if project_match else None,
        "discipline_code": discipline,
        "drawing_number": drawing_number,
        "revision": revision,
        "confidence": confidence,
        "source": "filename",
    }
