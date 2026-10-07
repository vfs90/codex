"""Run the native CLI against a slow loopback Responses fixture, then resize it.

This review-only harness supplies synthetic context usage, never account quotas.
Run with terminal-env/bin/python; add --no-color to exercise NO_COLOR.
"""
import argparse
import codecs
import fcntl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import threading
import time
import pyte

args = argparse.ArgumentParser()
args.add_argument('--no-color', action='store_true')
options = args.parse_args()
review = Path(__file__).resolve().parent.parent
captures = review / ('no-color' if options.no_color else 'color')
captures.mkdir(exist_ok=True)
requests = []


class Fixture(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        payload = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        body = json.loads(payload)
        # The native CLI also generates a title in a hidden structured thread.
        # Account for that existing feature instead of counting it as a resize
        # request or changing product configuration to disable it.
        is_title = 'Generate a concise, single-line task title' in json.dumps(body)
        requests.append({'path': self.path, 'kind': 'title' if is_title else 'main'})
        number = sum(request['kind'] == 'main' for request in requests)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Connection', 'close')
        self.end_headers()

        def event(value):
            self.wfile.write(('data: ' + json.dumps(value) + '\n\n').encode())
            self.wfile.flush()

        response_id = f'fixture-{number}'
        message_id = f'message-{number}'
        try:
            event({'type': 'response.created', 'response': {'id': response_id}})
            item = {'type': 'message', 'role': 'assistant', 'id': message_id,
                    'content': [{'type': 'output_text', 'text': ''}]}
            event({'type': 'response.output_item.added', 'item': item})
            parts = [json.dumps({'title': 'Test infobar'})] if is_title else (
                ['Fixture ready.\n'] if number == 1 else [f'Stream row {i:02}.\n' for i in range(90)])
            for part in parts:
                event({'type': 'response.output_text.delta', 'delta': part})
                if not is_title and number > 1:
                    time.sleep(0.2)
            item['content'][0]['text'] = ''.join(parts)
            event({'type': 'response.output_item.done', 'item': item})
            event({'type': 'response.completed', 'response': {'id': response_id,
                  'usage': {'input_tokens': 100000, 'output_tokens': 0, 'total_tokens': 100000}}})
        except (BrokenPipeError, ConnectionResetError):
            pass


server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
server.daemon_threads = True
threading.Thread(target=server.serve_forever, daemon=True).start()
work = review / 'smoke-work'
work.mkdir(exist_ok=True)
home = captures / 'smoke-home'
home.mkdir(exist_ok=True)
(home / 'config.toml').write_text('''
[tui]
animations = false
infobar = ["model-with-reasoning", "context-remaining", "five-hour-limit", "weekly-limit", "banked-resets"]
''')
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 160, 0, 0))
env = dict(os.environ, TERM='xterm-256color', CODEX_INFOBAR_TEST_HOME=str(home))
if options.no_color:
    env['NO_COLOR'] = '1'
    env.pop('FORCE_COLOR', None)
else:
    env.pop('NO_COLOR', None)
    env['FORCE_COLOR'] = '3'
provider = '{name="Loopback fixture",base_url="http://127.0.0.1:' + str(server.server_port) + '/v1",wire_api="responses",requires_openai_auth=false}'
proc = subprocess.Popen([
    str(review / 'run-local.sh'), '--strict-config', '-s', 'read-only', '-m', 'gpt-test',
    '-c', 'model_provider="infobar-preview"', '-c', 'model_context_window=256000',
    '-c', f'model_providers.infobar-preview={provider}',
], cwd=work, env=env, stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
os.close(slave)
captured = bytearray()


class InteractiveScreen(pyte.Screen):
    def write_process_input(self, data):
        os.write(master, data.encode())


screen = InteractiveScreen(160, 30)
stream = pyte.Stream(screen)
decoder = codecs.getincrementaldecoder('utf-8')('replace')
resize_screens = []
frames = []


def collect(seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if select.select([master], [], [], 0.05)[0]:
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            captured.extend(data)
            stream.feed(decoder.decode(data))
        if proc.poll() is not None:
            break


def visible():
    text = captured.decode('utf-8', 'replace')
    text = re.sub(r'\x1b\][^\x07]*(?:\x07|\x1b\\)', '', text)
    return re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', text)


def resize(width, height=30):
    screen.resize(lines=height, columns=width)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, 0, 0))
    os.kill(proc.pid, signal.SIGWINCH)


def capture_frame(width, height):
    frames.append({'width': width, 'height': height, 'rows': [
        [{'text': screen.buffer[y][x].data, 'fg': screen.buffer[y][x].fg,
          'bg': screen.buffer[y][x].bg, 'bold': screen.buffer[y][x].bold}
         for x in range(width)] for y in range(height)]})


try:
    collect(5)
    if 'trust' in visible().lower():
        os.write(master, b'\r')
        collect(3)
    startup = '\n'.join(screen.display[:5])
    assert 'Context unknown' in startup and '0/256K' not in startup, '\n'.join(screen.display)
    os.write(master, b'fixture')
    collect(0.25)
    os.write(master, b'\r')
    collect(4)
    display = '\n'.join(screen.display)
    # Native notifications use 95% of configured 256K for this fallback model.
    # Keep that actual usable capacity and the existing reserved-baseline %.
    assert '100K/243K' in display and '62% left' in display, display
    assert sum(request['kind'] == 'main' for request in requests) == 1, requests
    os.write(master, b'stream')
    collect(0.25)
    os.write(master, b'\r')
    collect(0.8)
    assert sum(request['kind'] == 'main' for request in requests) == 2, requests
    expected_requests = len(requests)
    os.write(master, b'draft')
    collect(0.2)
    for width in [160, 80, 50, 49, 48, 47, 46, 45, 44, 40, 24, 22, 20, 22, 24, 40, 80, 160]:
        resize(width)
        collect(0.4)
        display = '\n'.join(screen.display)
        bar = '\n'.join(screen.display[:5])
        assert '100K/243K' in bar and '62% left' in bar, display
        assert 'Resets 0' not in bar, bar
        assert 'draft' in display, display
        if width >= 80:
            assert '[' in bar and ']' in bar, bar
        if width <= 40:
            assert '[' not in bar, bar
        if options.no_color:
            header_rows = next(i + 1 for i, row in enumerate(screen.display[:5]) if '62% left' in row)
            assert all(screen.buffer[y][x].fg == 'default' and screen.buffer[y][x].bg == 'default'
                       for y in range(header_rows) for x in range(width)
                       if screen.buffer[y][x].data.strip()), bar
        resize_screens.append(f'{width}x30\n{display}')
        capture_frame(width, 30)
    # Cursor-positioned terminal diffs can reuse spaces rather than emit them.
    # Assert against the parsed screen, not raw bytes with ANSI removed.
    assert 'Stream row' in '\n'.join(screen.display), '\n'.join(screen.display)
    # Rapidly cross the single-meter boundary while the response streams.
    # Intermediate SIGWINCH events may coalesce; the final settled frame must
    # still contain the real context values and the unfinished prompt.
    burst_widths = list(range(60, 39, -1)) + list(range(40, 61))
    for width in burst_widths:
        resize(width)
        collect(0.035)
    resize(160)
    collect(0.4)
    display = '\n'.join(screen.display)
    assert '100K/243K' in display and '62% left' in display and 'draft' in display, display
    for height in [8, 3, 2, 1, 3, 8, 30]:
        resize(40, height)
        collect(0.25)
        assert proc.poll() is None, '\n'.join(screen.display)
        if height >= 8:
            assert 'draft' in '\n'.join(screen.display), '\n'.join(screen.display)
        resize_screens.append(f'40x{height}\n' + '\n'.join(screen.display))
    resize(160)
    collect(0.5)
    assert 'draft' in '\n'.join(screen.display)
    os.write(master, b'\x15/infobar')
    collect(0.25)
    os.write(master, b'\r')
    collect(0.8)
    assert 'ConfigureInfobar' in re.sub(r'\s+', '', visible()), visible()[-5000:]
    os.write(master, b'\x1b')
    collect(0.2)
    assert len(requests) == expected_requests, 'resize or typing unexpectedly triggered a model request'
    print(f'PASS: native CLI, {len(resize_screens)} settled size changes and {len(burst_widths)} rapid resizes, streaming loopback fixture, draft preserved, picker opened; NO_COLOR={options.no_color}; two conversation requests plus existing title generation; no extra requests during resize.')
finally:
    (captures / 'terminal-smoke.ansi').write_bytes(captured)
    (captures / 'terminal-smoke.txt').write_text(visible())
    (captures / 'terminal-resize.txt').write_text('\n\n'.join(resize_screens))
    (captures / 'frames.json').write_text(json.dumps(frames))
    (captures / 'request-summary.json').write_text(json.dumps(requests))
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    os.close(master)
    server.shutdown()
    server.server_close()
