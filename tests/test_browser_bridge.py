import importlib.util
import io
import pathlib
import subprocess
import tempfile
import unittest

MODULE = pathlib.Path(__file__).resolve().parents[1] / "test-env" / "browser_bridge.py"
spec = importlib.util.spec_from_file_location("browser_bridge", MODULE)
bridge = importlib.util.module_from_spec(spec)
if MODULE.exists():
    spec.loader.exec_module(bridge)


class FakeMouse:
    def __init__(self):
        self.clicks = []

    def click(self, x, y):
        self.clicks.append((x, y))


class FakePage:
    def __init__(self):
        self.mouse = FakeMouse()
        self.visits = []
        self.evaluations = []

    def goto(self, url, **kwargs):
        self.visits.append(url)

    def screenshot(self, path, **kwargs):
        pathlib.Path(path).write_bytes(b"jpeg")

    def evaluate(self, expression):
        self.evaluations.append(expression)
        return {"phase": "root", "clicks": 0}


class BridgeTests(unittest.TestCase):
    def test_parser_rejects_paths_nonfinite_outside_and_extra_fields(self):
        for command in ["OBSERVE\t../escape", "OBSERVE\t0", "OBSERVE\t01", "CLICK\tnan\t1", "CLICK\t800\t1", "CLICK\t1\t600", "CLICK\t-1\t0", "RESET\textra", "QUIT\t", "observe\t1", "CLICK\t1\t2\t3", "CLICK\t 1\t2"]:
            with self.subTest(command=command), self.assertRaises(ValueError):
                bridge.parse_command(command)
        self.assertEqual(bridge.parse_command("CLICK\t799.5\t0"), ("CLICK", 799.5, 0.0))
        self.assertEqual(bridge.parse_command("OBSERVE\t1"), ("OBSERVE", 1))

    def test_observation_uses_only_screenshot_and_tesseract(self):
        with tempfile.TemporaryDirectory() as directory:
            page = FakePage()
            calls = []
            def ocr(args, **kwargs):
                self.assertEqual(page.evaluations, [])
                calls.append((args, kwargs))
                pathlib.Path(args[2] + ".tsv").write_text("level\ttext\n", encoding="utf-8")
            target = bridge.Bridge(page, pathlib.Path(directory), "file:///fixture.html", ocr)
            out = io.StringIO()
            self.assertEqual(bridge.run_protocol(target, io.StringIO("OBSERVE\t1\nCLICK\t100\t200\nRESET\nQUIT\n"), out), 0)
            self.assertEqual(out.getvalue(), "READY\nOK\nOK\nOK\nOK\n")
            self.assertTrue((pathlib.Path(directory) / "frame-0001.jpg").is_file())
            self.assertTrue((pathlib.Path(directory) / "frame-0001.tsv").is_file())
            self.assertTrue((pathlib.Path(directory) / "oracle.json").is_file())
            self.assertEqual(page.mouse.clicks, [(100.0, 200.0)])
            self.assertEqual(page.visits, ["file:///fixture.html"])
            self.assertEqual(len(page.evaluations), 1)
            self.assertEqual(calls[0][0][-5:], ["-l", "eng", "--psm", "3", "tsv"])
            self.assertEqual(calls[0][1]["timeout"], 15)

    def test_runtime_error_stops_without_leaking_or_retrying(self):
        with tempfile.TemporaryDirectory() as directory:
            page = FakePage()
            def failing_ocr(*args, **kwargs):
                raise subprocess.TimeoutExpired("secret", 15, output="private OCR")
            target = bridge.Bridge(page, pathlib.Path(directory), "file:///fixture.html", failing_ocr)
            out = io.StringIO()
            self.assertEqual(bridge.run_protocol(target, io.StringIO("OBSERVE\t1\nCLICK\t2\t3\nQUIT\n"), out), 1)
            self.assertEqual(out.getvalue(), "READY\nERR\tOPERATION_FAILED\n")
            self.assertEqual(page.mouse.clicks, [])
            self.assertEqual(page.evaluations, [])

    def test_duplicate_observation_outputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = bridge.Bridge(FakePage(), pathlib.Path(directory), "file:///fixture.html")
            (pathlib.Path(directory) / "frame-0001.jpg").write_bytes(b"existing")
            with self.assertRaises(ValueError):
                target.observe(1)

    def test_bad_command_stops_before_later_input(self):
        with tempfile.TemporaryDirectory() as directory:
            page = FakePage()
            target = bridge.Bridge(page, pathlib.Path(directory), "file:///fixture.html")
            for command in ["CLICK\tnan\t0\n", "RESET\textra\n", "X" * 257 + "\n", "QUIT"]:
                with self.subTest(command=command):
                    out = io.StringIO()
                    self.assertEqual(bridge.run_protocol(target, io.StringIO(command + "CLICK\t1\t2\n"), out), 1)
                    self.assertEqual(out.getvalue(), "READY\nERR\tBAD_COMMAND\n")
                    self.assertEqual(page.mouse.clicks, [])
                    self.assertEqual(page.evaluations, [])

    def test_local_fixture_must_exist_and_be_html(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fixture = root / "fixture.html"
            fixture.write_text("<html></html>")
            self.assertEqual(bridge.validate_paths(root / "out", fixture), ((root / "out").resolve(), fixture.resolve().as_uri()))
            for missing in [root / "missing.html", root / "script.py"]:
                with self.assertRaises(ValueError):
                    bridge.validate_paths(root / "out", missing)
            (root / "out" / "existing").write_text("occupied")
            with self.assertRaises(ValueError):
                bridge.validate_paths(root / "out", fixture)


if __name__ == "__main__":
    unittest.main()
