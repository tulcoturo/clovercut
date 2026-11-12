"""Regression checks for incomplete and damaged interface translations."""
import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch
import locales


class TranslationChecks(unittest.TestCase):
    def test_shared_preset_labels_are_kept_in_the_translation_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            crates = pathlib.Path(directory)
            app = crates / "concat"
            (app / "src").mkdir(parents=True)
            (app / "ui").mkdir()
            project = crates / "concat-project" / "src"
            project.mkdir(parents=True)
            (project / "speed.rs").write_text(
                'label: "speedRamp.hero",\n// label: "test.comment"\n'
                '#[cfg(test)]\nlabel: "test.fake",\n', encoding="utf-8")
            with patch.object(locales, "CRATE", app):
                self.assertEqual(locales.code_keys(), {"speedRamp.hero"})

    def check_translation(self, english, translated, strict=True):
        with tempfile.TemporaryDirectory() as directory:
            folder = pathlib.Path(directory)
            (folder / "zz.json").write_text(json.dumps({"_": {"name": "Test"}, "test.message": translated}), encoding="utf-8")
            with patch.object(locales, "LOCALES", folder), contextlib.redirect_stdout(io.StringIO()):
                return locales.check({"test.message": english}, strict=strict)

    def test_untranslated_text_fails_strict_check(self):
        self.assertEqual(self.check_translation("Add to timeline", "Add to timeline"), 1)

    def test_placeholders_can_be_reordered(self):
        self.assertEqual(self.check_translation("{0} clips, {1} titles", "{1} titres, {0} clips"), 0)

    def test_missing_extra_or_duplicated_placeholders_fail(self):
        for translated in ["Valeur", "Valeur {1}", "Valeur {0} {0}"]:
            with self.subTest(translated=translated):
                self.assertEqual(self.check_translation("Value {0}", translated), 1)

    def test_empty_or_nontext_translation_fails(self):
        for translated in ["", "   ", 123, None]:
            with self.subTest(translated=translated):
                self.assertEqual(self.check_translation("Value", translated), 1)

    def test_reviewed_cognate_is_specific_to_language_and_exact_value(self):
        self.assertTrue(locales.same_is_fine("common.position", "Position", "de"))
        self.assertFalse(locales.same_is_fine("common.position", "Position", "ja"))
        self.assertFalse(locales.same_is_fine("common.position", "Position of clip", "de"))

    def test_product_names_and_units_are_not_missing_translations(self):
        self.assertTrue(locales.same_is_fine("sounds.concat", "OpenGameArt"))
        self.assertTrue(locales.same_is_fine("format.hoursMinutes", "{0} h {1} min"))

    def test_new_grading_parameter_cannot_silently_fall_back_to_manifest_english(self):
        problems = locales.grading_parity({"grading.temperature", "grading.lightness"}, {"temperature", "contrast"})
        self.assertEqual(problems, ["grading label missing from UI: grading.contrast"])

    def test_removed_grading_parameter_is_reported(self):
        problems = locales.grading_parity({"grading.temperature", "grading.lightness", "grading.obsolete"}, {"temperature"})
        self.assertEqual(problems, ["grading label no longer matches a parameter: grading.obsolete"])

    def test_grading_parameters_and_heading_have_complete_ui_inventory(self):
        self.assertEqual(locales.grading_parity({"grading.temperature", "grading.lightness", "common.effects"}, {"temperature"}), [])


if __name__ == "__main__":
    unittest.main()
