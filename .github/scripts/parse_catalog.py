from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import yaml


def entity_ref(entity: dict[str, Any]) -> str | None:
    """Build a Backstage entity ref, or None if kind/name are missing."""
    kind = entity.get("kind")
    metadata = entity.get("metadata") or {}
    name = metadata.get("name")
    if not kind or not name:
        return None
    namespace = metadata.get("namespace", "default")
    return f"{str(kind).lower()}:{namespace}/{name}"


def parse_definition(
    definition: Any, catalog_path: Path, ref: str
) -> tuple[dict[str, str], dict[str, str] | None, list[str]]:
    """Return (details, missing_file_entry_or_None, warnings) for a spec.definition value."""
    warnings: list[str] = []
    catalog_dir = catalog_path.parent

    if definition is None:
        return {"mode": "none"}, None, warnings

    if isinstance(definition, dict):
        # $text -> local file reference (any format Backstage can render, e.g. yaml)
        if "$text" in definition:
            raw_path = str(definition["$text"])
            if raw_path.startswith(("http://", "https://")):
                return {"mode": "url", "url": raw_path}, None, warnings
            resolved = (catalog_dir / raw_path).resolve()
            details = {"mode": "$text", "path": raw_path, "resolved_path": str(resolved)}
            if not resolved.is_file():
                return details, {
                    "entityRef": ref,
                    "path": raw_path,
                    "resolved_path": str(resolved),
                }, warnings
            return details, None, warnings

        # $json -> local JSON file reference
        if "$json" in definition:
            raw_path = str(definition["$json"])
            if raw_path.startswith(("http://", "https://")):
                return {"mode": "url", "url": raw_path}, None, warnings
            resolved = (catalog_dir / raw_path).resolve()
            details = {"mode": "$json", "path": raw_path, "resolved_path": str(resolved)}
            if not resolved.is_file():
                return details, {
                    "entityRef": ref,
                    "path": raw_path,
                    "resolved_path": str(resolved),
                }, warnings
            return details, None, warnings

        # $openapi -> remote URL reference
        if "$openapi" in definition:
            return {"mode": "$openapi", "url": str(definition["$openapi"])}, None, warnings

        # dict without a recognized $-key: treat as inline embedded spec
        return {"mode": "inline"}, None, warnings

    if isinstance(definition, str):
        if definition.startswith(("http://", "https://")):
            return {"mode": "url", "url": definition}, None, warnings
        # inline YAML/JSON spec written directly as a string block
        return {"mode": "inline"}, None, warnings

    if isinstance(definition, list):
        return {"mode": "inline"}, None, warnings

    warnings.append(f"{ref}: unsupported spec.definition type: {type(definition).__name__}")
    return {"mode": "unknown"}, None, warnings


def parse_file(
    catalog_path: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, str]], list[str]]:
    entities: list[dict[str, Any]] = []
    missing_files: list[dict[str, str]] = []
    warnings: list[str] = []

    try:
        with catalog_path.open(encoding="utf-8") as file:
            documents = list(yaml.safe_load_all(file))
    except (OSError, yaml.YAMLError) as error:
        warnings.append(f"{catalog_path}: failed to parse — {error}")
        return entities, missing_files, warnings

    for index, document in enumerate(documents, start=1):
        if document is None:
            continue
        if not isinstance(document, dict):
            warnings.append(f"{catalog_path} document {index}: expected a mapping — skipped")
            continue

        ref = entity_ref(document)
        if ref is None:
            warnings.append(
                f"{catalog_path} document {index}: missing kind or metadata.name — skipped"
            )
            continue

        spec = document.get("spec") or {}
        if not isinstance(spec, dict):
            warnings.append(f"{ref}: spec is not a mapping")
            spec = {}

        entity: dict[str, Any] = {
            "entityRef": ref,
            "kind": document.get("kind"),
            "name": (document.get("metadata") or {}).get("name"),
            "source": str(catalog_path),
        }

        # Only entities that declare a definition get one parsed (typically kind: API,
        # but any kind is supported so this doesn't silently drop non-API entities).
        if "definition" in spec:
            details, missing, definition_warnings = parse_definition(
                spec["definition"], catalog_path, ref
            )
            entity["definition"] = details
            if missing:
                missing_files.append(missing)
            warnings.extend(definition_warnings)

        entities.append(entity)

    return entities, missing_files, warnings


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog_files", nargs="+", type=Path)
    args = parser.parse_args()

    all_entities: list[dict[str, Any]] = []
    all_missing: list[dict[str, str]] = []
    all_warnings: list[str] = []

    for catalog_path in args.catalog_files:
        entities, missing, warnings = parse_file(catalog_path)
        all_entities.extend(entities)
        all_missing.extend(missing)
        all_warnings.extend(warnings)

    result = {
        "entities": all_entities,
        "entity_refs": [e["entityRef"] for e in all_entities],
        "missing_files": all_missing,
        "warnings": all_warnings,
    }
    json.dump(result, fp=sys.stdout, ensure_ascii=False, indent=2)
    print()


if __name__ == "__main__":
    main()