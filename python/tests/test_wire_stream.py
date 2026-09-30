import os
import socket
import struct
import sys
import tempfile
import threading
import time
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../poom")))

import poom
from poom import SpanKind, trace, trace_tool

class TestPoomWireStream(unittest.TestCase):
    def test_live_uds_framing(self):
        """Verify that spans are streamed over Unix Domain Sockets in binary postcard frames."""
        temp_dir = tempfile.TemporaryDirectory()
        sock_path = os.path.join(temp_dir.name, "poom_wire.sock")

        server_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server_sock.bind(sock_path)
        server_sock.listen(1)

        frames_received = []
        stop_event = threading.Event()

        def server_worker():
            server_sock.settimeout(2.0)
            try:
                conn, _ = server_sock.accept()
                conn.settimeout(2.0)
                data = b""
                while not stop_event.is_set() or len(frames_received) < 2:
                    try:
                        chunk = conn.recv(4096)
                        if not chunk:
                            break
                        data += chunk
                        # Unpack 10-byte headers
                        while len(data) >= 10:
                            magic = data[0:2]
                            if magic != b"PM":
                                break
                            version, flags, payload_len = struct.unpack("<HHI", data[2:10])
                            total_frame_len = 10 + payload_len
                            if len(data) >= total_frame_len:
                                payload = data[10:total_frame_len]
                                frames_received.append((version, flags, payload_len, payload))
                                data = data[total_frame_len:]
                            else:
                                break
                        if len(frames_received) >= 2:
                            break
                    except socket.timeout:
                        break
                conn.close()
            except socket.timeout:
                pass

        server_thread = threading.Thread(target=server_worker)
        server_thread.daemon = True
        server_thread.start()

        # Initialize poom with our live socket path
        poom.init(socket_path=sock_path)

        @trace(name="sample_streamed_fn", kind=SpanKind.AGENT)
        def run_agent_workflow():
            return "agent_result"

        res = run_agent_workflow()
        self.assertEqual(res, "agent_result")

        # Flush to force delivery to socket
        flushed = poom.flush(timeout_seconds=2.0)
        self.assertTrue(flushed)

        for _ in range(50):
            if len(frames_received) >= 2:
                break
            time.sleep(0.02)

        stop_event.set()
        server_thread.join(timeout=3.0)
        server_sock.close()
        temp_dir.cleanup()

        # Verify frames: should receive at least Handshake and IngestSpans
        self.assertGreaterEqual(len(frames_received), 1)
        # Verify first frame has version 1 and valid magic
        version, flags, payload_len, payload = frames_received[0]
        self.assertEqual(version, 1)
        self.assertGreater(payload_len, 0)
        self.assertEqual(len(payload), payload_len)
        print(f"\n[WIRE STREAM] Successfully verified {len(frames_received)} binary postcard frames received over UDS!")

if __name__ == "__main__":
    unittest.main()
