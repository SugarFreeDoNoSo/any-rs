# SPDX-License-Identifier: AGPL-3.0-only
"""Repository-wrapper regression tests; these are not runtime scenarios."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("repository_validator", ROOT / "tools/validate.py")
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class RepositoryValidationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        shutil.copytree(ROOT / "spec", self.root / "spec")
        shutil.copytree(ROOT / "docs", self.root / "docs")
        shutil.copytree(ROOT / "contracts", self.root / "contracts")
        for name in ("LICENSE", "README.md", "CONTRIBUTING.md"):
            shutil.copy2(ROOT / name, self.root / name)

    def test_original_sources_are_preserved(self):
        self.assertEqual(validator.validate_preservation(self.root),
                         {"source_artifacts_verified": 2, "imported_files_verified": 51})

    def test_changed_original_artifact_is_rejected(self):
        path = self.root / "spec/originals/my-framework-spec-v0.1.0.md"
        path.write_text("altered")
        with self.assertRaisesRegex(ValueError, "Original artifact changed"):
            validator.validate_preservation(self.root)

    def test_changed_extracted_contract_is_rejected(self):
        path = self.root / "spec" / validator.PACKAGE / "requirements.toml"
        path.write_text("altered")
        with self.assertRaisesRegex(ValueError, "Imported file changed"):
            validator.validate_preservation(self.root)

    def test_missing_extracted_file_is_rejected(self):
        (self.root / "spec" / validator.PACKAGE / "README.md").unlink()
        with self.assertRaisesRegex(ValueError, "Missing imported file"):
            validator.validate_preservation(self.root)

    def test_unexpected_imported_file_is_rejected(self):
        (self.root / "spec" / validator.PACKAGE / "extra.txt").write_text("extra")
        with self.assertRaisesRegex(ValueError, "Unexpected files"):
            validator.validate_preservation(self.root)

    def test_symlinked_imported_file_is_rejected(self):
        path = self.root / "spec" / validator.PACKAGE / "README.md"
        copy = self.root / "README-copy.md"
        copy.write_bytes(path.read_bytes())
        path.unlink()
        path.symlink_to(copy)
        with self.assertRaisesRegex(ValueError, "must not be a symlink"):
            validator.validate_preservation(self.root)

    def test_altered_manifest_is_rejected(self):
        (self.root / "spec/SHA256SUMS").write_text("f" * 64 + "  another.zip\n")
        with self.assertRaisesRegex(ValueError, "manifest differs"):
            validator.validate_preservation(self.root)

    def test_license_and_document_links_pass(self):
        self.assertEqual(validator.validate_scaffold(self.root)["license"], "AGPL-3.0-only")

    def test_changed_license_is_rejected(self):
        (self.root / "LICENSE").write_text("not the license")
        with self.assertRaisesRegex(ValueError, "LICENSE differs"):
            validator.validate_scaffold(self.root)

    def test_broken_document_link_is_rejected(self):
        (self.root / "docs/BROKEN.md").write_text("[missing](missing-file.md)")
        with self.assertRaisesRegex(ValueError, "Broken documentation link"):
            validator.validate_scaffold(self.root)

    def test_original_failure_is_not_hidden(self):
        failure = subprocess.CompletedProcess([], 1, "", "original validation error")
        with patch.object(validator.subprocess, "run", return_value=failure):
            with self.assertRaisesRegex(ValueError, "original validation error"):
                validator.run_original_validator(self.root)

    def test_original_nonpassing_report_is_rejected(self):
        failure = subprocess.CompletedProcess([], 0, '{"status":"failed"}', "")
        with patch.object(validator.subprocess, "run", return_value=failure):
            with self.assertRaisesRegex(ValueError, "did not report a pass"):
                validator.run_original_validator(self.root)

    def test_original_process_uses_package_directory_and_current_python(self):
        success = subprocess.CompletedProcess([], 0, '{"status":"pass"}', "")
        with patch.object(validator.subprocess, "run", return_value=success) as run:
            self.assertEqual(validator.run_original_validator(self.root), {"status": "pass"})
        run.assert_called_once_with(
            [validator.sys.executable, "tools/validate_spec.py"],
            cwd=self.root / "spec" / validator.PACKAGE,
            text=True, capture_output=True, check=False,
        )


if __name__ == "__main__":
    unittest.main()
