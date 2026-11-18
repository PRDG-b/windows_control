"""Independent oracle tests: python -m unittest discover -s tests -p 'test_*.py'."""
import importlib.util
import json
from pathlib import Path
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

spec = importlib.util.spec_from_file_location("test_app_server", Path(__file__).resolve().parents[1] / "test-app" / "server.py")
server_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(server_module)


class OracleTest(unittest.TestCase):
    def test_static_page_and_oracle_are_separate(self):
        server = server_module.make_server("127.0.0.1", 0)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        base = f"http://127.0.0.1:{server.server_port}"
        try:
            with urlopen(base + "/") as response:
                self.assertIn(b"Execute and Validate", response.read())
            state = {"phase": "completed", "clicks": 1, "scenario": "success"}
            with urlopen(Request(base + "/events", data=json.dumps(state).encode(), method="POST")) as response:
                self.assertEqual(response.status, 204)
            with urlopen(base + "/oracle") as response:
                self.assertEqual(json.load(response), state)
            with self.assertRaises(HTTPError) as error:
                urlopen(Request(base + "/events", data=b"{}", method="POST"))
            self.assertEqual(error.exception.code, 400)
            error.exception.close()
            with self.assertRaises(HTTPError) as error:
                urlopen(base + "/../Cargo.toml")
            error.exception.close()
        finally:
            server.shutdown()
            worker.join()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
