"""Verify that the site content and sidebar bounds transition smoothly during layout changes."""
import argparse
from http.server import ThreadingHTTPServer
from pathlib import Path
import subprocess
import threading

from native_app import NativeApp
from native_smoke import Fixture


def check_layout_transition(executable):
    app = NativeApp(executable, 'layout-transition')
    server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    server.handle_error = lambda *_: None
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    try:
        with (app.artifacts / 'app.log').open('w', encoding='utf-8') as log:
            app.start([origin + '/'], log)
            subprocess.run(
                [
                    'node',
                    str(Path(__file__).with_name('layout_transition.mjs')),
                    str(app.port),
                    str(app.content_port),
                    origin,
                    str(app.artifacts),
                ],
                check=True,
                timeout=30,
            )
    finally:
        try:
            app.close()
        finally:
            server.shutdown()
            server.server_close()
        print(f'Layout transition evidence: {app.artifacts}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', default='target/debug/veluno.exe')
    check_layout_transition(parser.parse_args().exe)
