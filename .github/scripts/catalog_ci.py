from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

import parse_catalog


@dataclass(frozen=True)
class Artifact:
    path: Path
    display_path: str
    format: str
    required_key: str


@dataclass(frozen=True)
class Config:
    repository_root: Path
    command: tuple[str, ...]
    artifacts: tuple[Artifact, ...]


def _mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a mapping")
    return value


def _reject_unknown(mapping: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(mapping) - allowed)
    if unknown:
        prefix = f"{label} "
        raise ValueError(f"unknown {prefix}keys: {', '.join(unknown)}")


def _repository_path(raw_path: str, repository_root: Path, label: str) -> Path:
    resolved = (repository_root / raw_path).resolve()
    try:
        resolved.relative_to(repository_root.resolve())
    except ValueError as error:
        raise ValueError(f"{label} escapes repository: {raw_path}") from error
    return resolved


def load_config(path: Path, repository_root: Path) -> Config:
    repository_root = repository_root.resolve()
    with path.open(encoding="utf-8") as file:
        root = _mapping(yaml.safe_load(file), "config")
    _reject_unknown(root, {"version", "openapi"}, "config")
    if root.get("version") != 1:
        raise ValueError("config version must be 1")
    openapi = _mapping(root.get("openapi"), "openapi")
    _reject_unknown(openapi, {"generate", "artifacts"}, "openapi")
    generate = _mapping(openapi.get("generate"), "openapi.generate")
    _reject_unknown(generate, {"command"}, "openapi.generate")
    command = generate.get("command")
    if not isinstance(command, list) or not command or not all(isinstance(item, str) and item for item in command):
        raise ValueError("openapi.generate.command must be a non-empty string array")
    raw_artifacts = openapi.get("artifacts")
    if not isinstance(raw_artifacts, list) or not raw_artifacts:
        raise ValueError("openapi.artifacts must be a non-empty array")
    artifacts: list[Artifact] = []
    seen: set[Path] = set()
    for index, raw in enumerate(raw_artifacts):
        item = _mapping(raw, f"openapi.artifacts[{index}]")
        _reject_unknown(item, {"path", "format", "required_key"}, f"openapi.artifacts[{index}]")
        display_path = item.get("path")
        if not isinstance(display_path, str) or not display_path:
            raise ValueError(f"openapi.artifacts[{index}].path must be a non-empty string")
        resolved_path = _repository_path(display_path, repository_root, "artifact")
        if resolved_path in seen:
            raise ValueError(f"duplicate artifact path: {display_path}")
        seen.add(resolved_path)
        artifact_format = item.get("format")
        if artifact_format not in {"json", "yaml"}:
            raise ValueError(f"unsupported artifact format: {artifact_format}")
        required_key = item.get("required_key")
        if not isinstance(required_key, str) or not required_key:
            raise ValueError(f"openapi.artifacts[{index}].required_key must be a non-empty string")
        artifacts.append(
            Artifact(
                path=resolved_path,
                display_path=display_path,
                format=artifact_format,
                required_key=required_key,
            )
        )
    return Config(repository_root, tuple(command), tuple(artifacts))


def generate_openapi(config: Config) -> None:
    subprocess.run(config.command, cwd=config.repository_root, check=True)


def artifact_paths(config: Config) -> tuple[Path, ...]:
    return tuple(artifact.path for artifact in config.artifacts)


def artifacts_changed(config: Config) -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain", "--", *(artifact.display_path for artifact in config.artifacts)],
        cwd=config.repository_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return bool(result.stdout.strip())


def validate_openapi(config: Config) -> None:
    for artifact in config.artifacts:
        if not artifact.path.is_file():
            raise ValueError(f"artifact not found: {artifact.display_path}")
        with artifact.path.open(encoding="utf-8") as file:
            document = json.load(file) if artifact.format == "json" else yaml.safe_load(file)
        if not isinstance(document, dict):
            raise ValueError(f"{artifact.display_path}: document must be a mapping")
        if not document.get(artifact.required_key):
            raise ValueError(f"{artifact.display_path}: missing required key: {artifact.required_key}")
        if not (document.get("info") or {}).get("title"):
            raise ValueError(f"{artifact.display_path}: missing info.title")
        if not isinstance(document.get("paths"), dict) or not document["paths"]:
            raise ValueError(f"{artifact.display_path}: has no paths")


def _catalog_files(repository_root: Path) -> list[Path]:
    excluded = {".git", ".venv", "target", "node_modules"}
    return sorted(
        path for path in repository_root.rglob("catalog-info.yaml")
        if not excluded.intersection(path.relative_to(repository_root).parts)
    )


def catalog_result(repository_root: Path, output_path: Path) -> dict[str, Any]:
    repository_root = repository_root.resolve()
    files = _catalog_files(repository_root)
    if not files:
        raise ValueError("no catalog-info.yaml files found")
    entities: list[dict[str, Any]] = []
    missing_files: list[dict[str, str]] = []
    warnings: list[str] = []
    for path in files:
        parsed, missing, found_warnings = parse_catalog.parse_file(path, repository_root)
        entities.extend(parsed)
        missing_files.extend(missing)
        warnings.extend(found_warnings)
    result = {
        "entities": entities,
        "entity_refs": [entity["entityRef"] for entity in entities],
        "missing_files": missing_files,
        "warnings": warnings,
    }
    output_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return result


def render_summary(result_path: Path) -> str:
    lines = ["### 📋 Backstage Catalog Sync", ""]
    try:
        result = json.loads(result_path.read_text(encoding="utf-8"))
        if not isinstance(result, dict):
            raise ValueError("result must be an object")
        entities = result.get("entities")
        warnings = result.get("warnings")
        missing_files = result.get("missing_files", [])
        if not isinstance(entities, list) or not isinstance(warnings, list) or not isinstance(missing_files, list):
            raise ValueError("entities, warnings, and missing_files must be arrays")
        for entity in entities:
            if not isinstance(entity, dict) or not isinstance(entity.get("entityRef"), str):
                raise ValueError("every entity must contain a string entityRef")
            definition = entity.get("definition")
            if definition is not None and not isinstance(definition, dict):
                raise ValueError("entity definition must be an object when present")
            if isinstance(definition, dict) and "mode" in definition and not isinstance(definition["mode"], str):
                raise ValueError("entity definition mode must be a string")
    except (OSError, json.JSONDecodeError, ValueError) as error:
        lines.append(f"⚠️ Catalog results unavailable: {error}")
        return "\n".join(lines) + "\n"
    lines.append("**Entities:**")
    for entity in entities:
        mode = (entity.get("definition") or {}).get("mode")
        suffix = f" _({mode})_" if mode else ""
        lines.append(f"- `{entity['entityRef']}`{suffix}")
    lines.append("")
    lines.append(f"**Warnings:** {len(warnings)}")
    lines.append(f"**Missing local definitions:** {len(missing_files)}")
    return "\n".join(lines) + "\n"


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("generate-openapi")
    subparsers.add_parser("validate-openapi")
    subparsers.add_parser("artifact-paths")
    subparsers.add_parser("artifacts-changed")
    catalog = subparsers.add_parser("catalog-result")
    catalog.add_argument("--output", type=Path, required=True)
    summary = subparsers.add_parser("summary")
    summary.add_argument("--result", type=Path, required=True)
    return parser


def main() -> None:
    args = _parser().parse_args()
    repository_root = args.repository.resolve()
    try:
        if args.command == "summary":
            print(render_summary(args.result), end="")
            return
        config = load_config(repository_root / ".github" / "catalog-ci.yaml", repository_root)
        if args.command == "generate-openapi":
            generate_openapi(config)
        elif args.command == "validate-openapi":
            validate_openapi(config)
            print(f"validated {len(config.artifacts)} OpenAPI artifact(s)")
        elif args.command == "artifact-paths":
            for artifact in config.artifacts:
                print(artifact.display_path)
        elif args.command == "artifacts-changed":
            raise SystemExit(0 if artifacts_changed(config) else 1)
        elif args.command == "catalog-result":
            result = catalog_result(repository_root, args.output)
            print(json.dumps(result, ensure_ascii=False, indent=2))
            if result["warnings"] or result["missing_files"]:
                raise SystemExit(1)
    except (OSError, ValueError, yaml.YAMLError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        print(f"catalog CI error: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
