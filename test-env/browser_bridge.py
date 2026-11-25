"""Container-only screenshot/OCR bridge; stdout is a strict agent protocol.

The agent receives image/TSV observations and coordinate input only. The sole
DOM read happens on QUIT and goes to the evaluator's separate oracle.json.
"""
import argparse
from contextlib import contextmanager
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import threading


VIEWPORT = {"width": 800, "height": 600}


def parse_command(line):
    fields = line.split("\t")
    if fields in (["RESET"], ["QUIT"]):
        return (fields[0],)
    if len(fields) == 2 and fields[0] == "OBSERVE" and re.fullmatch(r"[1-9][0-9]{0,5}", fields[1]):
        return ("OBSERVE", int(fields[1]))
    if len(fields) == 3 and fields[0] == "CLICK":
        if all(re.fullmatch(r"[0-9]+(?:\.[0-9]+)?", value) for value in fields[1:]):
            x, y = map(float, fields[1:])
            if math.isfinite(x) and math.isfinite(y) and 0 <= x < 800 and 0 <= y < 600:
                return ("CLICK", x, y)
    raise ValueError("bad command")


def validate_paths(output_dir, fixture):
    fixture = Path(fixture).resolve()
    if not fixture.is_file() or fixture.suffix.lower() != ".html":
        raise ValueError("invalid fixture")
    output_dir = Path(output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    if not output_dir.is_dir() or any(output_dir.iterdir()):
        raise ValueError("output must be empty")
    return output_dir, fixture.as_uri()


class Bridge:
    def __init__(self, page, output_dir, fixture_url, runner=subprocess.run, rerun=None):
        self.page = page
        self.output_dir = Path(output_dir).resolve()
        self.fixture_url = fixture_url
        self.runner = runner
        self.rerun = rerun

    def artifact(self, name):
        # Names are internal constants or derived from a validated decimal ID.
        if not re.fullmatch(r"(?:frame-[0-9]{4,6}\.(?:jpg|tsv)|oracle\.json)", name):
            raise ValueError("invalid output")
        path = self.output_dir / name
        if path.is_symlink() or path.exists() or path.resolve().parent != self.output_dir:
            raise ValueError("occupied output")
        return path

    def observe(self, frame_id):
        if not isinstance(frame_id, int) or not 1 <= frame_id <= 999999:
            raise ValueError("invalid frame")
        image = self.artifact(f"frame-{frame_id:04d}.jpg")
        self.artifact(f"frame-{frame_id:04d}.tsv")
        self.page.screenshot(path=str(image), type="jpeg", quality=95,
                             full_page=False, scale="css", timeout=10000)
        self.runner(["tesseract", str(image), str(image.with_suffix("")),
                     "-l", "eng", "--psm", "3", "tsv"],
                    check=True, timeout=15, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL)
        if not image.with_suffix(".tsv").is_file():
            raise RuntimeError("missing observation")
        if self.rerun:
            try:
                self.rerun.set_time_sequence("frame_id", frame_id)
                self.rerun.log("screen/image", self.rerun.EncodedImage(path=str(image)))
                boxes = []
                labels = []
                with open(image.with_suffix(".tsv"), "r", encoding="utf-8") as f:
                    for line in f:
                        parts = line.strip().split("\t")
                        if len(parts) >= 12 and parts[0] == "5":
                            text = parts[11].strip()
                            conf = float(parts[10])
                            if text and conf > 30:
                                x, y, w, h = map(int, parts[6:10])
                                boxes.append([x, y, w, h])
                                labels.append(text)
                if boxes:
                    self.rerun.log(
                        "screen/ocr",
                        self.rerun.Boxes2D(
                            array=boxes,
                            array_format=self.rerun.Box2DFormat.XYWH,
                            labels=labels,
                        ),
                    )
            except Exception:
                pass

    def execute(self, command):
        kind = command[0]
        if kind == "OBSERVE":
            self.observe(command[1])
        elif kind == "CLICK":
            if self.rerun:
                try:
                    self.rerun.log(
                        "screen/click",
                        self.rerun.Points2D(
                            [[command[1], command[2]]],
                            colors=[[255, 30, 30]],
                            radii=[8],
                        ),
                    )
                    self.rerun.log(
                        "events",
                        self.rerun.TextLog(f"CLICK ({command[1]:.1f}, {command[2]:.1f})"),
                    )
                except Exception:
                    pass
            self.page.mouse.click(command[1], command[2])
        elif kind == "RESET":
            if self.rerun:
                try:
                    self.rerun.log("events", self.rerun.TextLog("RESET to root fixture"))
                except Exception:
                    pass
            self.page.goto(self.fixture_url, wait_until="load", timeout=10000)
        elif kind == "QUIT":
            if self.rerun:
                try:
                    self.rerun.log("events", self.rerun.TextLog("QUIT - Exploration finished"))
                except Exception:
                    pass
            destination = self.artifact("oracle.json")
            oracle = self.page.evaluate("window.testOracle")
            # Evaluator data is never returned through stdout or OCR.
            with destination.open("x", encoding="utf-8") as handle:
                json.dump(oracle, handle, allow_nan=False)


@contextmanager
def operation_deadline(output):
    # Playwright mouse/evaluate and browser teardown have no per-call timeout.
    # A process watchdog also bounds those calls; no timed-out action is retried.
    def expired():
        try:
            output.write("ERR\tTIMEOUT\n")
            output.flush()
        finally:
            os._exit(2)
    timer = threading.Timer(28, expired)
    timer.daemon = True
    timer.start()
    try:
        yield
    finally:
        timer.cancel()


def run_protocol(target, source, output):
    output.write("READY\n")
    output.flush()
    while True:
        line = source.readline(257)
        if not line:
            output.write("ERR\tUNEXPECTED_EOF\n")
            output.flush()
            return 1
        try:
            if len(line) > 256 or not line.endswith("\n"):
                raise ValueError("invalid line")
            command = parse_command(line[:-1])
        except ValueError:
            output.write("ERR\tBAD_COMMAND\n")
            output.flush()
            return 1
        try:
            with operation_deadline(output):
                target.execute(command)
        except Exception:
            output.write("ERR\tOPERATION_FAILED\n")
            output.flush()
            return 1
        output.write("OK\n")
        output.flush()
        if command[0] == "QUIT":
            return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--rerun", default=None, help="Rerun gRPC endpoint")
    args = parser.parse_args()
    rerun_client = None
    if args.rerun:
        try:
            import rerun as rr
            rr.init("windows_control_browser", spawn=False)
            rr.connect_grpc(args.rerun)
            rerun_client = rr
        except Exception:
            rerun_client = None
    try:
        output_dir, fixture_url = validate_paths(args.output_dir, args.fixture)
        # Lazy import keeps protocol/parser tests runnable without Playwright.
        from playwright.sync_api import sync_playwright
        with operation_deadline(sys.stdout):
            playwright = sync_playwright().start()
            browser = playwright.chromium.launch(headless=True)
            context = browser.new_context(viewport=VIEWPORT, device_scale_factor=1,
                                          service_workers="block")
            # The known local fixture is the only permitted network request.
            context.route("**/*", lambda route: route.continue_()
                          if route.request.url == fixture_url else route.abort())
            page = context.new_page()
            page.set_default_timeout(10000)
            page.goto(fixture_url, wait_until="load", timeout=10000)
        result = run_protocol(
            Bridge(page, output_dir, fixture_url, rerun=rerun_client),
            sys.stdin,
            sys.stdout,
        )
        with operation_deadline(sys.stdout):
            browser.close()
            playwright.stop()
        return result
    except Exception:
        print("ERR\tSTARTUP_FAILED", flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
