#!/usr/bin/env python3
"""Extract Backstage entities and API definition references from catalog files."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import yaml


def entity_ref(entity: dict[str, Any]) -> str:
    kind = str(entity.get("kind", "unknown")).lower()
    metadata = entity.get("metadata") or {}
    namespace = metadata.get("namespace", "default")
    name = metadata.get("name", "unknown")
    return f"{kind}:{namespace}/{name}"


def definition_details(
    definition: Any, catalog_path: Path, ref: str
) -> tuple[dict[str, str], dict[str, str] | None, list[str]]:
    warnings: list[str] = []

    if definition is None:
        return {}, None, warnings

    if isinstance(definition, dict) and "$text" in definition:
        raw_path = str(definition["$text"])
        if raw_path.startswith(("http://", "https://")):
            return {"mode": "url", "url": raw_path}, None, warnings

        resolved = (catalog_path.parent / raw_path).resolve()
        details = {"mode": "file", "path": raw_path}
        if not resolved.is_file():
            missing = {
                "entityRef": ref,
                "path": raw_path,
                "resolved_path": str(resolved),
            }
            return details, missing, warnings
        return details, None, warnings

    if isinstance(definition, str) and definition.startswith(("http://", "https://")):
        return {"mode": "url", "url": definition}, None, warnings

    if isinstance(definition, (str, dict, list)):
        return {"mode": "inline"}, None, warnings

    warnings.append(f"{ref}: unsupported spec.definition type: {type(definition).__name__}")
    return {"mode": "unknown"}, None, warnings


def parse_catalog(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, str]], list[str]]:
    entities: list[dict[str, Any]] = []
    missing_files: list[dict[str, str]] = []
    warnings: list[str] = []

    try:
        with path.open(encoding="utf-8") as file:
            documents = list(yaml.safe_load_all(file))
    except (OSError, yaml.YAMLError) as error:
        return entities, missing_files, [f"{path}: {error}"]

    for index, document in enumerate(documents, start=1):
        if document is None:
            continue
        if not isinstance(document, dict):
            warnings.append(f"{path} document {index}: expected a mapping")
            continue

        ref = entity_ref(document)
        spec = document.get("spec") or {}
        if not isinstance(spec, dict):
            warnings.append(f"{ref}: spec is not a mapping")
            spec = {}

        details, missing, definition_warnings = definition_details(
            spec.get("definition"), path, ref
        )
        entity = {
            "entityRef": ref,
            "kind": document.get("kind"),
            "name": (document.get("metadata") or {}).get("name"),
            "source": str(path),
            "definition": details,
        }
        entities.append(entity)
        if missing:
            missing_files.append(missing)
        warnings.extend(definition_warnings)

    return entities, missing_files, warnings


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("catalog_files", nargs="+", type=Path)
    args = parser.parse_args()

    entities: list[dict[str, Any]] = []
    missing_files: list[dict[str, str]] = []
    warnings: list[str] = []

    for catalog_path in args.catalog_files:
        parsed, missing, parse_warnings = parse_catalog(catalog_path)
        entities.extend(parsed)
        missing_files.extend(missing)
        warnings.extend(parse_warnings)

    result = {
        "entities": entities,
        "entity_refs": [entity["entityRef"] for entity in entities],
        "missing_files": missing_files,
        "warnings": warnings,
    }
    json.dump(result, fp=sys.stdout, ensure_ascii=False, indent=2)
    print()


if __name__ == "__main__":
    main()
