"""Exercise the shipped Rust/WebView2 app against local fixtures and an isolated profile."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import io
import math
import os
from pathlib import Path
import subprocess
import threading
import time
import wave
import struct
import uuid


class Fixture(BaseHTTPRequestHandler):
    release_download = threading.Event()
    truncated_watch_attempts = 0

    def do_GET(self):
        path = self.path.split('?', 1)[0]
        if path == '/truncated-watch':
            Fixture.truncated_watch_attempts += 1
            if Fixture.truncated_watch_attempts == 1:
                body = b'<!doctype html><title>Watch fixture</title><body data-state="skeleton"><div id="watch-page-skeleton">Skeleton only</div>'
                self.wfile.write(
                    b'HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n'
                    b'Cache-Control: no-store\r\nContent-Length: 500000\r\nConnection: close\r\n\r\n'
                    + body
                )
                self.wfile.flush()
                time.sleep(0.1)
                return
            body = b'<!doctype html><title>Watch fixture</title><body data-state="ready"><div id="movie_player">Ready player</div><script>window.watchReady = true;</script></body>'
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path.startswith('/cache-recovery/'):
            if path == '/cache-recovery/worker.js':
                body = b'''self.addEventListener('install', event => event.waitUntil(self.skipWaiting()));
self.addEventListener('activate', event => event.waitUntil(self.clients.claim()));
self.addEventListener('fetch', event => {
  if (new URL(event.request.url).pathname !== '/cache-recovery/bootstrap.js') return;
  event.respondWith(caches.open('veluno-poisoned').then(async cache =>
    await cache.match(event.request) || fetch(event.request)));
});'''
                content_type = 'text/javascript'
            elif path == '/cache-recovery/bootstrap.js':
                body = b"window.fixtureReady = true; document.body.dataset.state = 'ready';"
                content_type = 'text/javascript'
            else:
                body = b'''<!doctype html><title>Cache recovery fixture</title><body data-state="starting">Getting ready...
<script>
(async () => {
  await navigator.serviceWorker.register('/cache-recovery/worker.js');
  await navigator.serviceWorker.ready;
  if (!navigator.serviceWorker.controller) await new Promise(resolve =>
    navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true }));
  const script = document.createElement('script');
  script.src = '/cache-recovery/bootstrap.js';
  script.onerror = () => { document.body.dataset.state = 'stalled'; };
  document.head.append(script);
})();
</script>'''
                content_type = 'text/html; charset=utf-8'
            self.send_response(200)
            self.send_header('Content-Type', content_type)
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path == '/redirect-download':
            self.send_response(302)
            self.send_header('Location', '/slow-download')
            self.end_headers()
            return
        if path in ('/hold-downloads', '/release-downloads'):
            if path == '/hold-downloads':
                Fixture.release_download.clear()
            else:
                Fixture.release_download.set()
            self.send_response(204)
            self.end_headers()
            return
        if path == '/tone.wav':
            buffer = io.BytesIO()
            with wave.open(buffer, 'wb') as audio:
                audio.setparams((1, 2, 8000, 0, 'NONE', 'not compressed'))
                audio.writeframes(b''.join(struct.pack('<h', int(2000 * math.sin(index * math.tau * 440 / 8000))) for index in range(8000)))
            body = buffer.getvalue()
            self.send_response(200)
            self.send_header('Content-Type', 'audio/wav')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path in ('/tests/ui.html', '/src/ui.html', '/src/start_page.html', '/src/new_tab.html', '/src/browser.js', '/assets/favicon.svg'):
            body = (Path(__file__).resolve().parent.parent / path.lstrip('/')).read_bytes()
            self.send_response(200)
            self.send_header('Content-Type', 'image/svg+xml' if path.endswith('.svg') else 'text/javascript; charset=utf-8' if path.endswith('.js') else 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path in ('/download', '/slow-download'):
            body = b'x' * 65536 if self.path == '/slow-download' else b'Veluno download fixture\n'
            self.send_response(200)
            self.send_header('Content-Type', 'application/octet-stream')
            self.send_header('Content-Disposition', 'attachment; filename="veluno-smoke.txt"')
        else:
            title = {'/': 'fixture', '/second': 'second', '/next': 'next', '/popup': 'popup'}.get(self.path, 'fixture')
            extra = "<script>setTimeout(()=>{history.pushState({},'', '/route');document.title='Veluno route'},300)</script>" if self.path == '/spa' else ''
            download = '/slow-download' if self.path == '/slow' else '/download'
            body = f'''<!doctype html><html><meta charset="utf-8"><title>Veluno {title}</title>
<body style="font:16px system-ui;padding:40px"><h1>Veluno {title}</h1>
<label>Keep this draft <input id="note"></label><p><a href="/next">Next page</a></p>
<p><a href="/popup" target="_blank">Open a new tab</a></p>
<a id="download" href="{download}">Download test file</a>{extra}</body></html>'''.encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        if self.path == '/slow-download':
            self.wfile.write(body[:32768])
            self.wfile.flush()
            self.release_download.wait(timeout=60)
            self.wfile.write(body[32768:])
        else:
            self.wfile.write(body)

    def log_message(self, *_):
        pass


def await_report(process, path):
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if path.exists():
            result = json.loads(path.read_text(encoding='utf-8'))
            print(json.dumps(result))
            if not result['success']:
                raise AssertionError(result)
            return
        if process.poll() is not None:
            raise AssertionError(f'Veluno exited with {process.returncode} before writing {path}')
        time.sleep(.1)
    raise AssertionError(f'Native test timed out; see {path.parent}')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', default='target/debug/veluno.exe')
    args = parser.parse_args()
    executable = Path(args.exe).resolve()
    artifacts = Path('target/native-smoke') / str(uuid.uuid4())
    artifacts.mkdir(parents=True)
    artifacts = artifacts.resolve()
    server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    environment = {**os.environ, 'VELUNO_PROFILE_DIR': str(artifacts / 'profile'),
                   'VELUNO_SMOKE_ORIGIN': origin, 'VELUNO_SMOKE_TEST': str(artifacts / 'first.json'),
                   'VELUNO_SMOKE_HOLD': '1'}
    process = None
    try:
        with (artifacts / 'first.log').open('w') as log:
            process = subprocess.Popen([str(executable), origin + '/'], env=environment, stdout=log, stderr=log)
            await_report(process, artifacts / 'first.json')
            session = json.loads((artifacts / 'profile/session.json').read_text(encoding='utf-8'))
            assert session['active_workspace_id'] == 2
            assert any(tab['is_pinned'] for tab in session['tabs'])
            secondary = {key: value for key, value in environment.items() if not key.startswith('VELUNO_SMOKE_')}
            secondary['VELUNO_SMOKE_TEST'] = environment['VELUNO_SMOKE_TEST']
            assert subprocess.run([str(executable), origin + '/second'], env=secondary, timeout=15).returncode == 0
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                updated = json.loads((artifacts / 'profile/session.json').read_text(encoding='utf-8'))
                if len(updated['tabs']) == len(session['tabs']) + 1:
                    break
                time.sleep(.1)
            else:
                raise AssertionError('Second launch did not hand its link to the running instance')

            process.kill()
            process.wait(timeout=10)
        time.sleep(1)
        environment.pop('VELUNO_SMOKE_HOLD')
        environment['VELUNO_SMOKE_TEST'] = str(artifacts / 'restored.json')
        environment['VELUNO_SMOKE_RESTORE'] = '1'
        with (artifacts / 'restored.log').open('w') as log:
            process = subprocess.Popen([str(executable)], env=environment, stdout=log, stderr=log)
            await_report(process, artifacts / 'restored.json')
            assert process.wait(timeout=10) == 0
        (artifacts / 'profile/session.json').write_text('{truncated', encoding='utf-8')
        environment['VELUNO_SMOKE_TEST'] = str(artifacts / 'recovered.json')
        with (artifacts / 'recovered.log').open('w') as log:
            process = subprocess.Popen([str(executable)], env=environment, stdout=log, stderr=log)
            await_report(process, artifacts / 'recovered.json')
            assert process.wait(timeout=10) == 0
        print(f'Headless native smoke tests passed, including link handoff and corrupt-session backup recovery. Evidence: {artifacts}')
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        server.shutdown()


if __name__ == '__main__':
    main()
