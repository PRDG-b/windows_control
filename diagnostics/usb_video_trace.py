"""Bounded Linux usbmon capture of the known video endpoint only; no HID input.

Run after sudo modprobe usbmon. Records bulk IN completions from endpoint 0x85
of the Actions Micro capture device backing /dev/video0. Other events are not
saved. Kernel ABI: https://docs.kernel.org/usb/usbmon.html
"""
import collections
import ctypes
import errno
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time


def selected(header, bus, device):
    return (len(header) == 64 and header[8] == ord('C') and header[9] == 3
            and header[10] == 0x85 and header[11] == device
            and struct.unpack_from('=H', header, 12)[0] == bus)


def main():
    import fcntl
    if os.geteuid() != 0:
        raise SystemExit('Run this diagnostic with sudo; do not change usbmon permissions.')
    device_path = Path('/sys/class/video4linux/video0/device').resolve()
    usb = next((p for p in (device_path, *device_path.parents) if (p / 'busnum').exists() and (p / 'devnum').exists()), None)
    if usb is None or (usb / 'idVendor').read_text().strip() != '1de1' or (usb / 'idProduct').read_text().strip() != 'f115':
        raise SystemExit('Expected capture device 1de1:f115 backing video0; refusing to trace another device.')
    bus, device = int((usb / 'busnum').read_text()), int((usb / 'devnum').read_text())
    monitor = os.open(f'/dev/usbmon{bus}', os.O_RDONLY | os.O_NONBLOCK)
    output = Path(tempfile.mkdtemp(prefix='windows-control-usbtrace-'))
    owner = int(os.environ.get('SUDO_UID', '0'))
    group = int(os.environ.get('SUDO_GID', '0'))

    class GetArgument(ctypes.Structure):
        _fields_ = [('header', ctypes.c_void_p), ('data', ctypes.c_void_p), ('alloc', ctypes.c_size_t)]

    header = ctypes.create_string_buffer(64)
    payload = ctypes.create_string_buffer(1024 * 1024)
    argument = GetArgument(ctypes.addressof(header), ctypes.addressof(payload), len(payload))
    request = (1 << 30) | (ctypes.sizeof(argument) << 16) | (0x92 << 8) | 10
    counts = collections.Counter()
    packets = total_bytes = clipped = 0
    child = None
    try:
        # Temporary per-reader ring; released when this fd closes.
        fcntl.ioctl(monitor, 0x9204, 16 * 1024 * 1024)
        child = subprocess.Popen([
            'v4l2-ctl', '-d', '/dev/video0', '--verbose', '--stream-mmap=4',
            '--stream-skip=4', '--stream-count=60',
            '--stream-to-hdr=' + str(output / 'v4l2-frames.bin'),
        ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 8
        with (output / 'video-packets.bin').open('xb') as packets_file:
            while time.monotonic() < deadline:
                try:
                    fcntl.ioctl(monitor, request, bytes(argument))
                except OSError as exc:
                    if exc.errno == errno.EAGAIN:
                        time.sleep(0.001)
                        continue
                    if exc.errno == errno.EINTR:
                        continue
                    raise
                raw_header = header.raw
                if not selected(raw_header, bus, device):
                    continue
                length = struct.unpack_from('=I', raw_header, 36)[0]
                status = struct.unpack_from('=i', raw_header, 28)[0]
                clipped += length > len(payload)
                length = min(length, len(payload))
                # Record: uint32 payload length, 64-byte usbmon header, payload.
                packets_file.write(struct.pack('<I', length))
                packets_file.write(raw_header)
                packets_file.write(payload.raw[:length])
                counts[status] += 1
                packets += 1
                total_bytes += length
        stats = bytearray(8)
        fcntl.ioctl(monitor, (2 << 30) | (8 << 16) | (0x92 << 8) | 3, stats, True)
        queued, dropped = struct.unpack('=II', stats)
        forced_stop = child.poll() is None
        if forced_stop:
            child.terminate()
        try:
            _, log = child.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            child.kill()
            _, log = child.communicate()
        (output / 'v4l2.log').write_bytes(log)
        report = dict(directory=str(output), bus=bus, device=device, endpoint='0x85',
                      packets=packets, payload_bytes=total_bytes, status_counts=dict(counts),
                      clipped_packets=clipped, monitor_dropped=dropped, monitor_queued=queued,
                      capture_exit=child.returncode, forced_stop=forced_stop)
        (output / 'report.json').write_text(json.dumps(report, indent=2))
        print(json.dumps(report, indent=2), flush=True)
    finally:
        if child is not None and child.poll() is None:
            child.kill()
            child.communicate()
        os.close(monitor)
        # Keep artifacts private to the invoking sudo user, not world-readable.
        for path in output.iterdir():
            os.chmod(path, 0o600)
            os.chown(path, owner, group)
        os.chown(output, owner, group)


if __name__ == '__main__':
    main()
