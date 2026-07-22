from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

import parse_catalog
import catalog_ci


class WorkflowTests(unittest.TestCase):
    def test_self_hosted_workflow_does_not_upload_rust_build_cache(self) -> None:
        workflow = (SCRIPT_DIR.parent / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("Swatinem/rust-cache", workflow)


class ParseCatalogTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name).resolve()
        self.catalog = self.root / "catalog-info.yaml"
        (self.root / "spec.yaml").write_text("openapi: 3.1.0\n", encoding="utf-8")
        (self.root / "spec.json").write_text('{"openapi":"3.1.0"}\n', encoding="utf-8")

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def parse_definition(self, definition: object):
        return parse_catalog.parse_definition(
            definition, self.catalog, "api:default/example", self.root
        )

    def test_definition_patterns(self) -> None:
        cases = [
            ({"$text": "./spec.yaml"}, "$text"),
            ({"$json": "./spec.json"}, "$json"),
            ({"$openapi": "https://example.test/openapi.yaml"}, "$openapi"),
            ({"openapi": "3.1.0"}, "inline"),
            ("openapi: 3.1.0", "inline"),
            ([{"openapi": "3.1.0"}], "inline"),
            (None, "none"),
        ]
        for definition, expected_mode in cases:
            with self.subTest(definition=definition):
                details, missing, warnings = self.parse_definition(definition)
                self.assertEqual(expected_mode, details["mode"])
                self.assertIsNone(missing)
                self.assertEqual([], warnings)

    def test_url_text_and_json_do_not_require_local_files(self) -> None:
        for key in ("$text", "$json"):
            details, missing, warnings = self.parse_definition(
                {key: "https://example.test/spec.yaml"}
            )
            self.assertEqual({"mode": "url", "url": "https://example.test/spec.yaml"}, details)
            self.assertIsNone(missing)
            self.assertEqual([], warnings)

    def test_missing_local_file_is_reported(self) -> None:
        details, missing, warnings = self.parse_definition({"$text": "./missing.yaml"})
        self.assertEqual("$text", details["mode"])
        self.assertEqual("./missing.yaml", missing["path"])
        self.assertEqual([], warnings)

    def test_local_path_outside_repository_is_rejected(self) -> None:
        details, missing, warnings = self.parse_definition({"$text": "../outside.yaml"})
        self.assertEqual("$text", details["mode"])
        self.assertIsNone(missing)
        self.assertEqual(
            ["api:default/example: local definition escapes repository: ../outside.yaml"],
            warnings,
        )

    def test_multi_document_catalog_accepts_non_api_definition(self) -> None:
        self.catalog.write_text(
            """apiVersion: backstage.io/v1alpha1
kind: Component
metadata: {name: app}
spec: {definition: {$text: ./spec.yaml}}
---
apiVersion: backstage.io/v1alpha1
kind: Resource
metadata: {name: db}
spec: {type: database}
""",
            encoding="utf-8",
        )
        entities, missing, warnings = parse_catalog.parse_file(self.catalog, self.root)
        self.assertEqual(["component:default/app", "resource:default/db"], [e["entityRef"] for e in entities])
        self.assertEqual([], missing)
        self.assertEqual([], warnings)

    def test_malformed_yaml_returns_warning(self) -> None:
        self.catalog.write_text("metadata: [\n", encoding="utf-8")
        entities, missing, warnings = parse_catalog.parse_file(self.catalog, self.root)
        self.assertEqual([], entities)
        self.assertEqual([], missing)
        self.assertEqual(1, len(warnings))
        self.assertIn("failed to parse", warnings[0])


class CatalogCiTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name).resolve()
        (self.root / ".github").mkdir()
        self.config_path = self.root / ".github" / "catalog-ci.yaml"

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def write_config(self, data: dict) -> None:
        self.config_path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")

    def valid_config(self) -> dict:
        return {
            "version": 1,
            "openapi": {
                "generate": {"command": ["tool", "generate"]},
                "artifacts": [
                    {"path": "openapi.json", "format": "json", "required_key": "openapi"}
                ],
            },
        }

    def write_valid_spec(self, path: str = "openapi.json") -> None:
        (self.root / path).write_text(
            json.dumps({"openapi": "3.1.0", "info": {"title": "API"}, "paths": {"/health": {}}}),
            encoding="utf-8",
        )

    def test_load_config_accepts_version_one_and_array_command(self) -> None:
        self.write_config(self.valid_config())
        config = catalog_ci.load_config(self.config_path, self.root)
        self.assertEqual(("tool", "generate"), config.command)
        self.assertEqual((self.root / "openapi.json",), catalog_ci.artifact_paths(config))

    def test_load_config_rejects_unknown_keys(self) -> None:
        data = self.valid_config()
        data["unknown"] = True
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "unknown config keys: unknown"):
            catalog_ci.load_config(self.config_path, self.root)

    def test_load_config_rejects_wrong_version(self) -> None:
        data = self.valid_config()
        data["version"] = 2
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "config version must be 1"):
            catalog_ci.load_config(self.config_path, self.root)

    def test_load_config_rejects_empty_or_string_command(self) -> None:
        for command in ([], "tool generate"):
            data = self.valid_config()
            data["openapi"]["generate"]["command"] = command
            self.write_config(data)
            with self.assertRaisesRegex(ValueError, "openapi.generate.command must be a non-empty string array"):
                catalog_ci.load_config(self.config_path, self.root)

    def test_load_config_rejects_duplicate_or_escaping_artifact_paths(self) -> None:
        data = self.valid_config()
        data["openapi"]["artifacts"].append(dict(data["openapi"]["artifacts"][0]))
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "duplicate artifact path: openapi.json"):
            catalog_ci.load_config(self.config_path, self.root)
        data = self.valid_config()
        data["openapi"]["artifacts"][0]["path"] = "../outside.json"
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "artifact escapes repository: ../outside.json"):
            catalog_ci.load_config(self.config_path, self.root)

    def test_load_config_rejects_canonical_duplicate_artifact_paths(self) -> None:
        data = self.valid_config()
        duplicate = dict(data["openapi"]["artifacts"][0])
        duplicate["path"] = "./openapi.json"
        data["openapi"]["artifacts"].append(duplicate)
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "duplicate artifact path: ./openapi.json"):
            catalog_ci.load_config(self.config_path, self.root)

    def test_load_config_rejects_unsupported_format(self) -> None:
        data = self.valid_config()
        data["openapi"]["artifacts"][0]["format"] = "toml"
        self.write_config(data)
        with self.assertRaisesRegex(ValueError, "unsupported artifact format: toml"):
            catalog_ci.load_config(self.config_path, self.root)

    def test_validate_openapi_accepts_json_and_yaml(self) -> None:
        data = self.valid_config()
        data["openapi"]["artifacts"].append(
            {"path": "openapi.yaml", "format": "yaml", "required_key": "openapi"}
        )
        self.write_config(data)
        self.write_valid_spec()
        (self.root / "openapi.yaml").write_text(
            "openapi: 3.1.0\ninfo: {title: API}\npaths: {/health: {}}\n", encoding="utf-8"
        )
        catalog_ci.validate_openapi(catalog_ci.load_config(self.config_path, self.root))

    def test_validate_openapi_rejects_missing_required_structure(self) -> None:
        self.write_config(self.valid_config())
        for document, message in [
            ({"info": {"title": "API"}, "paths": {"/": {}}}, "missing required key: openapi"),
            ({"openapi": "3.1.0", "paths": {"/": {}}}, "missing info.title"),
            ({"openapi": "3.1.0", "info": {"title": "API"}, "paths": {}}, "has no paths"),
        ]:
            (self.root / "openapi.json").write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, message):
                catalog_ci.validate_openapi(catalog_ci.load_config(self.config_path, self.root))

    def test_catalog_result_finds_nested_catalogs(self) -> None:
        nested = self.root / "nested"
        nested.mkdir()
        (nested / "catalog-info.yaml").write_text(
            "apiVersion: backstage.io/v1alpha1\nkind: Component\nmetadata: {name: app}\nspec: {type: service}\n",
            encoding="utf-8",
        )
        output = self.root / "result.json"
        result = catalog_ci.catalog_result(self.root, output)
        self.assertEqual(["component:default/app"], result["entity_refs"])
        self.assertEqual(result, json.loads(output.read_text(encoding="utf-8")))

    def test_summary_handles_valid_missing_and_malformed_results(self) -> None:
        result = self.root / "result.json"
        self.assertIn("Catalog results unavailable", catalog_ci.render_summary(result))
        result.write_text("not json", encoding="utf-8")
        self.assertIn("Catalog results unavailable", catalog_ci.render_summary(result))
        result.write_text(
            json.dumps({"entities": [], "warnings": [], "missing_files": [{"path": "missing"}]}),
            encoding="utf-8",
        )
        self.assertIn("Warnings:** 0", catalog_ci.render_summary(result))
        self.assertIn("Missing local definitions:** 1", catalog_ci.render_summary(result))

    def test_summary_handles_structurally_malformed_json(self) -> None:
        result = self.root / "result.json"
        for document in (
            [],
            {"entities": [None]},
            {"entities": [{}]},
            {
                "entities": [{"entityRef": "api:default/example", "definition": "bad"}],
                "warnings": [],
                "missing_files": [],
            },
        ):
            result.write_text(json.dumps(document), encoding="utf-8")
            self.assertIn("Catalog results unavailable", catalog_ci.render_summary(result))


if __name__ == "__main__":
    unittest.main()
