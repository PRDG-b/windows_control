"""Analyze usb_video_trace artifacts offline. Supports one EOF-marked UVC
payload per frame, split across full bulk URBs and a final short URB.
Refuses incomplete records, data-bearing error URBs and unfinished payloads.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def read_packets(blob):
    packets, offset = [], 0
    while offset < len(blob):
        if offset + 68 > len(blob):
            raise ValueError('Truncated USB record header')
        size = struct.unpack_from('<I', blob, offset)[0]
        header = blob[offset + 4:offset + 68]
        end = offset + 68 + size
        if end > len(blob):
            raise ValueError('Truncated USB record data')
        actual, captured = struct.unpack_from('=II', header, 32)
        if size != actual or size != captured:
            raise ValueError('USB capture length differs from actual transfer')
        packets.append(dict(
            status=struct.unpack_from('=i', header, 28)[0],
            time=struct.unpack_from('=q', header, 16)[0] + struct.unpack_from('=i', header, 24)[0] / 1e6,
            data=blob[offset + 68:end],
        ))
        offset = end
    return packets


def reconstruct(packets, bulk_size):
    frames, payload = [], bytearray()
    started = None
    for packet in packets:
        data = packet['data']
        if packet['status']:
            if data or payload:
                raise ValueError('Error URB with data or unfinished payload')
            continue
        if len(data) > bulk_size:
            raise ValueError('Transfer exceeds bulk URB size')
        if started is None:
            started = packet['time']
        payload.extend(data)
        if len(data) == bulk_size:
            continue
        if len(payload) < 2:
            raise ValueError('Missing UVC header')
        size, flags = payload[:2]
        if not 2 <= size <= len(payload) or not flags & 0x80 or not flags & 0x02:
            raise ValueError('Expected complete EOF-marked UVC payload')
        frames.append(dict(data=bytes(payload[size:]), flags=flags, start=started, end=packet['time']))
        payload.clear()
        started = None
    if payload:
        raise ValueError('Unfinished bulk payload')
    return frames


def read_v4l2(blob):
    frames, offset, magic = [], 0, blob[:4]
    while offset < len(blob):
        if offset + 8 > len(blob) or blob[offset:offset + 4] != magic:
            raise ValueError('Invalid V4L2 frame header')
        size = struct.unpack_from('>I', blob, offset + 4)[0]
        end = offset + 8 + size
        if end > len(blob):
            raise ValueError('Truncated V4L2 frame')
        frames.append(blob[offset + 8:end])
        offset = end
    return frames


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--bulk-size', type=int, default=16384)
    args = parser.parse_args()
    capture = json.loads((args.directory / 'report.json').read_text())
    if any(capture[k] for k in ('clipped_packets', 'monitor_dropped', 'monitor_queued', 'capture_exit', 'forced_stop')):
        raise ValueError('Capture reports loss or incomplete termination')
    packets = read_packets((args.directory / 'video-packets.bin').read_bytes())
    frames = reconstruct(packets, args.bulk_size)
    v4l2 = read_v4l2((args.directory / 'v4l2-frames.bin').read_bytes())
    invalid = lambda data: not data.startswith(b'\xff\xd8') or not data.endswith(b'\xff\xd9')
    usb_bad = [i for i, f in enumerate(frames) if invalid(f['data'])]
    v4l2_bad = [i for i, data in enumerate(v4l2) if invalid(data)]
    offsets = [offset for offset in range(len(frames) - len(v4l2) + 1)
               if all(data == frames[i + offset]['data'] for i, data in enumerate(v4l2))]
    start = packets[0]['time']
    good = [f for f in frames if not invalid(f['data'])]
    result = dict(
        usb_frames=len(frames), usb_bad_marker_frames=usb_bad,
        v4l2_frames=len(v4l2), v4l2_bad_marker_frames=v4l2_bad,
        exact_contiguous_usb_offsets=offsets,
        bad_usb_all_eof=all(frames[i]['flags'] & 2 != 0 for i in usb_bad),
        bad_usb_any_error_flag=any(frames[i]['flags'] & 0x40 != 0 for i in usb_bad),
        first_good_usb_start_seconds=round(good[0]['start'] - start, 6) if good else None,
        last_data_seconds=round(max(p['time'] for p in packets if p['data']) - start, 6),
        nonzero_urbs=[dict(status=p['status'], seconds=round(p['time'] - start, 6), bytes=len(p['data'])) for p in packets if p['status']],
        bad_v4l2_sha256=[hashlib.sha256(v4l2[i]).hexdigest() for i in v4l2_bad],
    )
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
