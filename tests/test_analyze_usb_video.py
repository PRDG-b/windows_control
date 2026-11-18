import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location("analyze_usb_video", Path(__file__).resolve().parents[1] / "diagnostics" / "analyze_usb_video.py")
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)


class TraceParserTest(unittest.TestCase):
    def packet(self, data):
        header = bytearray(64)
        struct.pack_into('=q', header, 16, 100)
        struct.pack_into('=II', header, 32, len(data), len(data))
        return struct.pack('<I', len(data)) + header + data

    def test_reassembles_payload_before_removing_uvc_header(self):
        jpeg = b'\xff\xd8abcdefgh\xff\xd9'
        payload = b'\x02\x82' + jpeg
        records = self.packet(payload[:8]) + self.packet(payload[8:])
        packets = analysis.read_packets(records)
        frames = analysis.reconstruct(packets, 8)
        self.assertEqual(frames[0]['data'], jpeg)

    def test_truncated_records_are_rejected(self):
        with self.assertRaises(ValueError):
            analysis.read_packets(self.packet(b'abc')[:-1])
        with self.assertRaises(ValueError):
            analysis.read_v4l2(b'abcd' + struct.pack('>I', 10) + b'abc')


if __name__ == '__main__':
    unittest.main()
