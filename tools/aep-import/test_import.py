"""Synthetic converter regressions; uses unittest and the required py-aep package."""

import contextlib
import io
import json
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import py_aep

import aep2effectcraft as converter
import aepx


def make_project(path):
    """Generate our own input without shipping binary or Adobe-authored fixtures."""
    app = py_aep.new()
    comp = app.project.root_folder.add_comp("Main", 320, 180, 1, 2, 30)
    layer = comp.add_solid([1, 0, 0], "Red", 320, 180, 1, 2)
    position = layer.property("ADBE Transform Group").property("ADBE Position")
    position.set_value_at_time(0, [80, 90, 0])
    position.set_value_at_time(1, [160, 90, 0])
    marker = py_aep.MarkerValue("start ✓", chapter="chapter ✓")
    marker.duration = 0.25
    comp.marker_property.set_value_at_time(0.5, marker)
    layer.marker.set_value_at_time(0.75, py_aep.MarkerValue("layer ✓"))
    app.project.save(path)


class ConverterTests(unittest.TestCase):
    def test_missing_dependency_reports_the_interpreter_and_setup(self):
        result = subprocess.run(
            [sys.executable, "-S", str(Path(converter.__file__).resolve())],
            capture_output=True,
            encoding="utf-8",
            check=False,
        )
        self.assertEqual(result.returncode, 3)
        self.assertIn("py-aep could not be imported", result.stderr)
        self.assertIn(sys.executable, result.stderr)
        self.assertIn("Python environment", result.stderr)

    def test_fractional_frame_times_keep_their_precision(self):
        time = 1001 / 30000
        self.assertEqual(float(converter.js(time)), time)

    def test_nonfinite_values_are_rejected_instead_of_changing_to_zero(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value), self.assertRaisesRegex(converter.Unsupported, "non-finite"):
                converter.js(value)

    def test_marker_objects_preserve_strings_and_numbers(self):
        marker = SimpleNamespace(
            comment='start ✓ "one"',
            duration=0.25,
            chapter="chapter ✓",
            url="",
            frame_target="",
            cue_point_name="",
            label=2,
        )
        prop = SimpleNamespace(property_value_type=py_aep.PropertyValueType.MARKER)
        literal = converter.value_js(prop, marker)
        self.assertTrue(literal.startswith("MV("))
        self.assertEqual(json.loads(literal[3:-1])["comment"], marker.comment)
        self.assertEqual(json.loads(literal[3:-1])["duration"], 0.25)

    def test_synthetic_project_keeps_comp_and_layer_markers(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.aep"
            make_project(path)
            project = converter.load(str(path)).project
            code = converter.Converter(project, str(path)).run(path.name)
        self.assertEqual(code.count("m.setValueAtTime("), 2)
        self.assertIn('"chapter": "chapter', code)
        self.assertIn("setValuesAtTimes([0, 1]", code)
        self.assertIn("openInViewer()", code)

    def test_title_cannot_escape_a_generated_comment(self):
        project = SimpleNamespace(compositions=[], footages=[], items={}, root_folder=None)
        generated = converter.Converter(project, "test.aep")
        with patch.object(generated, "folders"):
            code = generated.run("project\nthrow new Error('injected');\n.aep")
        self.assertNotIn("\nthrow new Error('injected');\n", code)

    def test_deep_property_tree_reports_a_limit(self):
        generated = converter.Converter(None, "test.aep")
        group = SimpleNamespace(properties=[], match_name="group", property_type=py_aep.PropertyType.NAMED_GROUP)
        for _ in range(70):
            group = SimpleNamespace(
                properties=[group], match_name="group", property_type=py_aep.PropertyType.NAMED_GROUP
            )
        generated.walk(group, "layer", "deep tree", 0)
        self.assertIn("nesting", generated.notes[0])

    def test_folder_cycle_is_rejected(self):
        folder_type = type("FolderItem", (), {})
        root = folder_type()
        root.id = 1
        root.parent_folder = root
        project = SimpleNamespace(root_folder=root, items={1: root})
        with self.assertRaisesRegex(ValueError, "cyclic"):
            converter.Converter(project, "test.aep").folders()

    def test_conversion_error_has_an_actionable_message(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.aep"
            path.write_bytes(b"synthetic invalid project")
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                status = converter.main([str(path)])
        self.assertEqual(status, 2)
        self.assertIn("could not read the project", stderr.getvalue())

    def test_dash_prefixed_path_works_through_stdout(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "-synthetic.aep"
            make_project(path)
            result = subprocess.run(
                [sys.executable, str(Path(converter.__file__).resolve()), "--", path.name],
                cwd=directory,
                capture_output=True,
                encoding="utf-8",
                check=True,
            )
        self.assertIn("Imported -synthetic.aep", result.stdout)

    def test_aepx_rejects_a_different_document(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.aepx"
            path.write_text("<different/>", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "not an After Effects XML project"):
                aepx.aepx_to_aep_bytes(path)

    def test_aepx_rejects_excessive_nesting_with_an_actionable_error(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "deep.aepx"
            path.write_text(
                "<AfterEffectsProject>" + "<Fold>" * 70 + "</Fold>" * 70 + "</AfterEffectsProject>",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "nesting depth"):
                aepx.aepx_to_aep_bytes(path)

    def test_aepx_repackages_utf8_and_binary_chunks(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.aepx"
            path.write_text(
                '<AfterEffectsProject><Fold><string>ação</string><test bdata="010203"/></Fold></AfterEffectsProject>',
                encoding="utf-8",
            )
            data = aepx.aepx_to_aep_bytes(path)
        text = "ação".encode("utf-8")
        chunks = b"Utf8" + struct.pack(">I", len(text)) + text + b"test\0\0\0\3\1\2\3\0"
        body = b"Egg!LIST" + struct.pack(">I", len(chunks) + 4) + b"Fold" + chunks
        self.assertEqual(data, b"RIFX" + struct.pack(">I", len(body)) + body)


if __name__ == "__main__":
    unittest.main()
