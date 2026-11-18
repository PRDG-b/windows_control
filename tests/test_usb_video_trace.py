import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location("usb_video_trace", Path(__file__).resolve().parents[1] / "diagnostics" / "usb_video_trace.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class VideoFilterTest(unittest.TestCase):
    def header(self, kind=b'C', transfer=3, endpoint=0x85, device=4, bus=1):
        header = bytearray(64)
        header[8:12] = bytes([kind[0], transfer, endpoint, device])
        struct.pack_into("=H", header, 12, bus)
        return header

    def test_only_selected_video_completion_is_recorded(self):
        self.assertTrue(probe.selected(self.header(), 1, 4))
        for header in [self.header(kind=b'S'), self.header(transfer=1), self.header(endpoint=0x81), self.header(device=5), self.header(bus=2)]:
            self.assertFalse(probe.selected(header, 1, 4))


if __name__ == '__main__':
    unittest.main()
