"""HTML fullscreen in a tab must take over the Windows screen, not just the page area."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import subprocess
import threading
import time

from native_app import NativeApp

PAGE = b"""<!doctype html><title>Fullscreen fixture</title>
<body style="margin:0;background:#000"><div id="player" style="width:640px;height:360px;background:#222;color:#fff">Player</div></body>"""

user32 = ctypes.WinDLL('user32', use_last_error=True)
callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.GetParent.argtypes = [wintypes.HWND]
user32.GetParent.restype = wintypes.HWND
user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
user32.EnumChildWindows.argtypes = [wintypes.HWND, callback_type, wintypes.LPARAM]
user32.MonitorFromWindow.argtypes = [wintypes.HWND, wintypes.DWORD]
user32.MonitorFromWindow.restype = wintypes.HANDLE
WS_VISIBLE = 0x10000000
GWL_STYLE = -16
MONITOR_DEFAULTTONEAREST = 2


class MonitorInfo(ctypes.Structure):
    _fields_ = [
        ('cbSize', wintypes.DWORD),
        ('rcMonitor', wintypes.RECT),
        ('rcWork', wintypes.RECT),
        ('dwFlags', wintypes.DWORD),
    ]


user32.GetMonitorInfoW.argtypes = [wintypes.HANDLE, ctypes.POINTER(MonitorInfo)]


def rect_tuple(rect):
    return (rect.left, rect.top, rect.right, rect.bottom)


def window_geometry(process_id):
    """The app's main window, its monitor, and its visible child panels in screen pixels."""
    roots = []

    @callback_type
    def root(hwnd, _):
        pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == process_id:
            rect = wintypes.RECT()
            user32.GetClientRect(hwnd, ctypes.byref(rect))
            roots.append((rect.right, hwnd))
        return True

    user32.EnumWindows(root, 0)
    window = max(roots)[1]
    frame = wintypes.RECT()
    user32.GetWindowRect(window, ctypes.byref(frame))
    info = MonitorInfo(cbSize=ctypes.sizeof(MonitorInfo))
    user32.GetMonitorInfoW(user32.MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), ctypes.byref(info))
    client = wintypes.RECT()
    user32.GetClientRect(window, ctypes.byref(client))
    origin = wintypes.POINT(0, 0)
    user32.ClientToScreen(window, ctypes.byref(origin))
    panels = []

    @callback_type
    def child(hwnd, _):
        if user32.GetParent(hwnd) == window and user32.GetWindowLongW(hwnd, GWL_STYLE) & WS_VISIBLE:
            rect = wintypes.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(rect))
            if rect.right > rect.left and rect.bottom > rect.top:
                panels.append(rect_tuple(rect))
        return True

    user32.EnumChildWindows(window, child, 0)
    return {
        'window': rect_tuple(frame),
        'monitor': rect_tuple(info.rcMonitor),
        'work': rect_tuple(info.rcWork),
        'client': (origin.x, origin.y, origin.x + client.right, origin.y + client.bottom),
        'panels': panels,
    }


def covers_screen(state):
    """The window frame and its client area both match the monitor."""
    return state['window'] == state['monitor'] and state['client'] == state['monitor']


def fills_screen(state):
    """Fullscreen video covers the monitor and the page is the only panel left on screen."""
    return covers_screen(state) and state['panels'] == [state['client']]


def is_windowed(state):
    return state['window'] != state['monitor'] and len(state['panels']) >= 2


def is_maximized(state):
    """A maximized window keeps the work area, so the taskbar stays visible below it."""
    return state['window'] != state['monitor'] and state['client'] == state['work']


class FullscreenFixture(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Content-Length', str(len(PAGE)))
        self.end_headers()
        self.wfile.write(PAGE)

    def log_message(self, *_):
        pass


def check_content_fullscreen(executable):
    app = NativeApp(executable, 'content-fullscreen')
    server = ThreadingHTTPServer(('127.0.0.1', 0), FullscreenFixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    command = Path(__file__).with_name('native_command.mjs')
    process = None

    def sidebar(expression):
        result = subprocess.run(['node', str(command), str(app.port), expression],
                                check=True, capture_output=True, text=True, timeout=20)
        return json.loads(result.stdout)

    def page(expression):
        result = subprocess.run(['node', str(command), str(app.content_port), expression, origin + '/'],
                                check=True, capture_output=True, text=True, timeout=20)
        return json.loads(result.stdout)

    def wait_until(description, probe):
        deadline = time.monotonic() + 15
        while True:
            ok, detail = probe()
            if ok:
                return detail
            if time.monotonic() >= deadline:
                raise AssertionError(f'{description}. Last observed: {detail}')
            time.sleep(.1)

    def geometry(predicate):
        return lambda: (lambda state: (predicate(state), state))(window_geometry(process.pid))

    def enter_video_fullscreen():
        entered = page("document.getElementById('player').requestFullscreen().then(() => document.fullscreenElement?.id ?? null)")
        assert entered == 'player', f'Page did not enter HTML fullscreen: {entered!r}'
        wait_until('The video went fullscreen in the page, but the window did not fill the screen', geometry(fills_screen))
        viewport = page('[innerWidth, innerHeight, screen.width, screen.height]')
        assert abs(viewport[0] - viewport[2]) <= 1 and abs(viewport[1] - viewport[3]) <= 1, f'Page viewport does not match the screen: {viewport}'
        return viewport

    def wait_for_windowed(description):
        return wait_until(description, geometry(is_windowed))

    try:
        with (app.artifacts / 'app.log').open('w', encoding='utf-8') as log:
            process = app.start([origin + '/'], log)
            wait_until('The fixture page did not load', lambda: (
                bool(sidebar('currentState.tabs.find(tab=>tab.id===currentState.active_tab_id)?.title==="Fullscreen fixture" && !currentState.is_loading')),
                None,
            ))
            assert not fills_screen(window_geometry(process.pid)), 'Window was fullscreen before the test'

            # Leaving video fullscreen from the page restores the browser layout.
            viewport = enter_video_fullscreen()
            page('document.exitFullscreen().then(() => document.fullscreenElement === null)')
            wait_for_windowed('Leaving video fullscreen did not restore the browser layout')

            # A maximized window keeps its work-area client size, so fullscreen has to restore it
            # first or the page stops at the taskbar instead of covering it.
            sidebar('window.ipc.postMessage(JSON.stringify({action:"window_maximize"}));true')
            wait_until('The window did not maximize', geometry(is_maximized))
            enter_video_fullscreen()
            page('document.exitFullscreen().then(() => document.fullscreenElement === null)')
            wait_until('Leaving video fullscreen did not return to a maximized window', geometry(is_maximized))

            # F11 from a maximized window covers the screen too, and comes back maximized.
            sidebar('window.ipc.postMessage(JSON.stringify({action:"toggle_fullscreen"}));true')
            wait_until('F11 from a maximized window did not cover the screen', geometry(covers_screen))
            sidebar('window.ipc.postMessage(JSON.stringify({action:"toggle_fullscreen"}));true')
            wait_until('Leaving F11 fullscreen did not return to a maximized window', geometry(is_maximized))
            sidebar('window.ipc.postMessage(JSON.stringify({action:"window_maximize"}));true')
            wait_for_windowed('The window did not return to its windowed frame')

            # F11 also leaves video fullscreen, and the page must leave its fullscreen too.
            enter_video_fullscreen()
            sidebar('window.ipc.postMessage(JSON.stringify({action:"toggle_fullscreen"}));true')
            wait_for_windowed('F11 during video fullscreen did not restore the window')
            assert page('document.fullscreenElement === null') is True, 'Page stayed fullscreen after F11'

            # Switching tabs during video fullscreen leaves it, so the window never stays fullscreen on the wrong tab.
            enter_video_fullscreen()
            sidebar('window.ipc.postMessage(JSON.stringify({action:"new_tab",url:"veluno://newtab"}));true')
            wait_for_windowed('Switching tabs during video fullscreen left the window fullscreen')
            assert page('document.fullscreenElement === null') is True, 'Hidden tab stayed in fullscreen after the tab switch'
        print(f'Video fullscreen filled the screen ({viewport[2]}x{viewport[3]}), and F11 and tab switches leave it. Evidence: {app.artifacts}')
    finally:
        app.close()
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    check_content_fullscreen('target/debug/veluno.exe')
