"""Drive the real sidebar IPC with WebView2 debugging in an isolated hidden app."""
import argparse
import copy
import ctypes
from ctypes import wintypes
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import subprocess
import sys
import threading
import time
from urllib.request import urlopen

from native_smoke import Fixture
from native_app import NativeApp
from native_cache import check_cache_recovery
from tab_recovery import check_recovery
from tab_input import check_tab_input
from update_restore import check_update_restore
from content_fullscreen import check_content_fullscreen
from topbar_animation import check_topbar_animation
from layout_transition import check_layout_transition


def visible_panels(process_id):
    """Visible child WebViews of the main window as (x, width, height) in client pixels."""
    user32 = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.GetParent.argtypes = [wintypes.HWND]
    user32.GetParent.restype = wintypes.HWND
    user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.ScreenToClient.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.EnumChildWindows.argtypes = [wintypes.HWND, callback_type, wintypes.LPARAM]
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
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
    parent = max(roots)[1]
    panels = []

    @callback_type
    def child(hwnd, _):
        if user32.GetParent(hwnd) == parent and user32.GetWindowLongW(hwnd, -16) & 0x10000000:
            rect = wintypes.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(rect))
            origin = wintypes.POINT(rect.left, rect.top)
            user32.ScreenToClient(parent, ctypes.byref(origin))
            if rect.bottom - rect.top > 100:
                panels.append((origin.x, rect.right - rect.left, rect.bottom - rect.top))
        return True

    user32.EnumChildWindows(parent, child, 0)
    return panels


def sidebar_side(process_id):
    panels = visible_panels(process_id)
    if len(panels) != 2:
        return None
    sidebar, content = sorted(panels, key=lambda panel: panel[1])
    return 'right' if sidebar[0] > content[0] else 'left'


def assert_sidebar_above_content(process_id, palette_open=False):
    user32 = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.GetParent.argtypes = [wintypes.HWND]
    user32.GetParent.restype = wintypes.HWND
    user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.EnumChildWindows.argtypes = [wintypes.HWND, callback_type, wintypes.LPARAM]
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
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
    parent = max(roots)[1]
    children = []

    @callback_type
    def child(hwnd, _):
        if user32.GetParent(hwnd) == parent and user32.GetWindowLongW(hwnd, -16) & 0x10000000:
            rect = wintypes.RECT()
            user32.GetClientRect(hwnd, ctypes.byref(rect))
            children.append((rect.right, rect.bottom))
        return True

    user32.EnumChildWindows(parent, child, 0)
    panels = [(i, width, height) for i, (width, height) in enumerate(children) if height > 100]
    assert len(panels) == (3 if palette_open else 2), children
    sidebar = min(panels, key=lambda panel: panel[1])[0]
    content = max(panels, key=lambda panel: panel[1])[0]
    assert sidebar < content, f'Sidebar hover area is behind page content: {children}'
    topbars = [i for i, (width, height) in enumerate(children) if width > 260 and 0 < height <= 100]
    assert len(topbars) == 1, children
    assert sidebar < topbars[0], f'Floating sidebar is behind the top bar: {children}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', default='target/debug/veluno.exe')
    args = parser.parse_args()
    check_topbar_animation(args.exe)
    check_layout_transition(args.exe)
    check_cache_recovery(args.exe)
    check_tab_input(args.exe)
    check_recovery(args.exe)
    check_update_restore(args.exe)
    check_content_fullscreen(args.exe)
    app = NativeApp(args.exe, 'native-regressions')
    artifacts, profile = app.artifacts, app.profile
    port, content_port = app.port, app.content_port
    server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    checks = []
    process = None

    def state():
        try:
            return json.loads((profile / 'session.json').read_text(encoding='utf-8'))
        except (OSError, ValueError):
            return None

    def active(snapshot):
        return next(tab for tab in snapshot['tabs'] if tab['id'] == snapshot['active_tab_id'])

    def wait(predicate, description):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            snapshot = state()
            if snapshot and predicate(snapshot):
                return snapshot
            if process.poll() is not None:
                raise AssertionError(f'App exited before {description}')
            time.sleep(.1)
        raise AssertionError(f'Timed out: {description}. State: {state()}')

    def sidebar(expression):
        result = subprocess.run(['node', str(Path(__file__).with_name('native_command.mjs')), str(port), expression],
                                timeout=20, capture_output=True, text=True, encoding='utf-8')
        if result.returncode:
            raise AssertionError(f'Native sidebar command failed:\n{result.stderr}')
        return json.loads(result.stdout)

    def command(action, **payload):
        message = json.dumps({'action': action, **payload})
        sidebar(f'window.ipc.postMessage({json.dumps(message)});true')

    def content(url, expression, text=None):
        result = subprocess.run(['node', str(Path(__file__).with_name('native_command.mjs')), str(content_port), expression, url, *([text] if text else [])],
                                timeout=20, capture_output=True, text=True, encoding='utf-8')
        if result.returncode:
            raise AssertionError(f'Content command failed:\n{result.stderr}')
        return json.loads(result.stdout)

    def switch(id):
        return sidebar(f'''(async()=>{{
            const started=performance.now();
            window.ipc.postMessage(JSON.stringify({{action:'switch_tab',id:{id}}}));
            while(performance.now()-started<10000) {{
                const tab=currentState.tabs.find(tab=>tab.id==={id});
                if(currentState.active_tab_id==={id} && !tab.is_loading && !tab.is_sleeping) return performance.now()-started;
                await new Promise(resolve=>setTimeout(resolve,20));
            }}
            throw Error('Tab {id} did not wake');
        }})()''')

    def loaded(url):
        return wait(lambda snapshot: active(snapshot)['url'] == url and not active(snapshot)['is_loading'], f'load {url}')

    try:
        with (artifacts / 'app.log').open('w', encoding='utf-8') as log:
            process = app.start([origin + '/'], log)
            loaded(origin + '/')
            sidebar('document.activeElement.blur(); window.ipc.postMessage(JSON.stringify({action:"toggle_sidebar"})); true')
            sidebar('(async()=>{await new Promise(resolve=>setTimeout(resolve,400)); return true})()')
            command('new_tab', url=origin + '/second')
            loaded(origin + '/second')
            assert_sidebar_above_content(process.pid)
            stop_monitor = threading.Event()
            stacking_errors = []

            def monitor_stacking():
                while not stop_monitor.is_set():
                    try:
                        assert_sidebar_above_content(process.pid)
                    except Exception as error:
                        stacking_errors.append(str(error))
                        return
                    stop_monitor.wait(.016)

            monitor = threading.Thread(target=monitor_stacking)
            monitor.start()
            try:
                animation = sidebar('''(async()=>{
                    const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
                    const widths=[];
                    const sample=async ms=>{
                        const end=performance.now()+ms;
                        while(performance.now()<end) { widths.push(innerWidth); await wait(16); }
                    };
                    await wait(400);
                    const closedWidth=innerWidth;
                    for(let i=0;i<3;i++) {
                        document.documentElement.dispatchEvent(new MouseEvent('mouseenter'));
                        await sample(400);
                        if(!currentState.sidebar_peek || document.body.classList.contains('collapsed')) throw Error('Sidebar did not reveal on hover');
                        document.documentElement.dispatchEvent(new MouseEvent('mouseleave'));
                        await sample(600);
                        if(currentState.sidebar_peek) throw Error('Sidebar did not hide after leaving');
                    }
                    document.documentElement.dispatchEvent(new MouseEvent('mouseenter'));
                    await sample(80);
                    document.documentElement.dispatchEvent(new MouseEvent('mouseleave'));
                    await sample(300);
                    document.documentElement.dispatchEvent(new MouseEvent('mouseenter'));
                    await sample(400);
                    if(!currentState.sidebar_peek) throw Error('Reversing a hide must reveal the sidebar');
                    document.documentElement.dispatchEvent(new MouseEvent('mouseleave'));
                    await sample(600);
                    return {closedWidth,widths};
                })()''')
            finally:
                stop_monitor.set()
                monitor.join(timeout=5)
            assert not stacking_errors, stacking_errors
            assert animation['closedWidth'] >= 260, animation
            assert all(width == animation['closedWidth'] for width in animation['widths']), animation
            assert_sidebar_above_content(process.pid)
            sidebar('document.documentElement.dispatchEvent(new MouseEvent("mouseenter"));true')
            sidebar('(async()=>{await new Promise(resolve=>setTimeout(resolve,400));document.getElementById("btn-add-tab").click();return true})()')
            sidebar('(async()=>{await new Promise(resolve=>setTimeout(resolve,500));if(!currentState.sidebar_peek)throw Error("New tab lost sidebar hover");return true})()')
            assert_sidebar_above_content(process.pid, palette_open=True)
            command('submit_new_tab', url=origin + '/second')
            loaded(origin + '/second')
            assert sidebar('currentState.sidebar_peek'), 'Opening a site must retain sidebar hover'
            sidebar('document.getElementById("btn-add-tab").click();true')
            sidebar('(async()=>{await new Promise(resolve=>setTimeout(resolve,400));return true})()')
            assert_sidebar_above_content(process.pid, palette_open=True)
            sidebar('document.documentElement.dispatchEvent(new MouseEvent("mouseleave"));true')
            wait(lambda _: not sidebar('currentState.sidebar_peek'), 'sidebar hides after leaving while search is open')
            command('dismiss_new_tab')
            command('toggle_sidebar')
            checks.append('New tab search leaves the hovered sidebar above its overlay and hides it only after pointer leave')
            checks.append('Hover reveal, hide, and reversal keep the native sidebar width stable and above new pages')
            command('navigate', url=origin + '/second')
            loaded(origin + '/second')
            command('navigate', url=origin)
            loaded(origin + '/')
            checks.append('An address without a trailing slash finishes loading')

            command('navigate', url=origin + '/spa')
            wait(lambda snapshot: active(snapshot)['url'] == origin + '/route' and active(snapshot)['title'] == 'Veluno route', 'pushState URL update')
            command('back')
            loaded(origin + '/')
            command('forward')
            loaded(origin + '/route')
            checks.append('pushState updates the saved URL and native Back and Forward still work')

            command('navigate', url='http://127.0.0.1:1/unreachable')
            wait(lambda snapshot: active(snapshot)['error'] and not active(snapshot)['is_loading'], 'native error page')
            command('back')
            recovered = loaded(origin + '/route')
            assert active(recovered)['error'] is None
            checks.append('Back leaves a failed navigation and clears its error')

            command('new_tab', url=origin + '/second')
            temporary = active(loaded(origin + '/second'))['id']
            command('sleep_inactive_tabs')
            command('close_tab', id=temporary)
            replacement = loaded(origin + '/route')
            assert not active(replacement)['is_sleeping']
            checks.append('Closing the active tab wakes its sleeping replacement')

            pinned = replacement['active_tab_id']
            command('toggle_pin', id=pinned)
            command('create_profile', name='School')
            other = wait(lambda snapshot: snapshot['active_workspace_id'] == 2, 'second workspace')['active_tab_id']
            command('switch_tab', id=pinned)
            command('new_tab', url=origin + '/slow')
            downloader = active(loaded(origin + '/slow'))['id']
            content(origin + '/slow', "document.getElementById('download').click();true")
            wait(lambda _: sidebar('currentState.active_downloads') == 1, 'active download')
            command('new_tab', url=origin + '/second')
            kept = active(loaded(origin + '/second'))['id']
            command('new_tab', url=origin + '/next')
            discard = active(loaded(origin + '/next'))['id']
            command('switch_tab', id=kept)
            command('clear_unpinned_tabs')
            remaining = wait(lambda snapshot: not any(tab['id'] == discard for tab in snapshot['tabs']), 'production Close Other Tabs')
            assert {tab['id'] for tab in remaining['tabs']} == {pinned, other, downloader, kept}
            assert remaining['active_tab_id'] == kept
            assert sidebar('currentState.active_downloads') == 1
            assert remaining['can_restore_closed_tab']
            command('restore_closed_tab')
            loaded(origin + '/next')
            checks.append('Close Other Tabs preserves pins, other workspaces, the active tab and a real download, with undo')
            Fixture.release_download.set()
            wait(lambda _: sidebar('currentState.active_downloads') == 0, 'download completion')
            assert (profile / 'smoke-download.txt').read_bytes() == b'x' * 65536
            subprocess.run(['node', str(Path(__file__).with_name('native_ui.mjs')), str(port), str(content_port), origin, str(artifacts)],
                           check=True, timeout=120)
            subprocess.run(['node', str(Path(__file__).with_name('native_downloads.mjs')), str(port), str(content_port), origin, str(artifacts)],
                           check=True, timeout=90)
            checks.append('Download-only tabs close without cancelling transfers; source documents, drafts, redirects, and Home survive')
            checks.append('Native keyboard, pointer selection, site permission decisions, default browser state, and DOM checks at desktop and minimum sizes')
            assert sidebar_side(process.pid) == 'left', visible_panels(process.pid)
            command('set_setting', key='sidebar_right', value=True)
            deadline = time.monotonic() + 10
            while sidebar_side(process.pid) != 'right':
                assert time.monotonic() < deadline, f'Sidebar did not move right: {visible_panels(process.pid)}'
                time.sleep(.1)
            assert sidebar('document.body.classList.contains("right")')
            command('set_setting', key='sidebar_right', value=False)
            deadline = time.monotonic() + 10
            while sidebar_side(process.pid) != 'left':
                assert time.monotonic() < deadline, f'Sidebar did not return left: {visible_panels(process.pid)}'
                time.sleep(.1)
            checks.append('The sidebar moves to the right edge and back live from Settings')

            source = next(tab['id'] for tab in sidebar('currentState.tabs') if tab['url'] == origin + '/next')
            command('duplicate_tab', id=source)
            duplicate = loaded(origin + '/next')['active_tab_id']
            assert duplicate != source
            command('move_tab_to_workspace', id=duplicate, workspace_id=2)
            wait(lambda snapshot: any(tab['id'] == duplicate and tab['workspace_id'] == 2 for tab in snapshot['tabs']),
                 'moved duplicate reaches disk')
            command('close_tab', id=duplicate)
            wait(lambda snapshot: all(tab['id'] != duplicate for tab in snapshot['tabs']), 'duplicate closes')
            command('expand_popup', url=origin + '/expanded')
            expanded = loaded(origin + '/expanded')['active_tab_id']
            command('close_tab', id=expanded)
            wait(lambda snapshot: all(tab['id'] != expanded for tab in snapshot['tabs']), 'expanded tab closes')
            checks.append('Duplicated tabs, workspace moves, and popup expansion persist through the session writer')

            seed = sidebar('currentState')
            capacity = []
            for count in (100, 300, 500):
                app.stop()
                time.sleep(1)
                snapshot = copy.deepcopy(seed)
                template = snapshot['tabs'][0]
                snapshot['tabs'] = [{**template, 'id': id, 'workspace_id': 1, 'title': f'Capacity {id}',
                                     'url': f'{origin}/capacity/{id}', 'is_pinned': id == 1,
                                     'is_loading': False, 'error': None, 'is_sleeping': True}
                                    for id in range(1, count + 1)]
                snapshot.update(active_tab_id=count, active_workspace_id=1, closed_tabs=[], active_downloads=0)
                (profile / 'session.json').write_text(json.dumps(snapshot), encoding='utf-8')
                Fixture.release_download.clear()
                started = time.monotonic()
                process = app.start([], log)
                wait(lambda _: sidebar('currentState.tabs.length') == count, 'capacity sidebar ready')
                loaded(f'{origin}/capacity/{count}')
                startup_ms = (time.monotonic() - started) * 1000
                with urlopen(f'http://127.0.0.1:{content_port}/json/list', timeout=5) as response:
                    pages = json.load(response)
                assert len([page for page in pages if page['type'] == 'page']) == 1, 'restore must create only the active WebView'
                latencies = [switch(1), switch(2)]
                content(f'{origin}/capacity/2', "document.getElementById('note').focus();true", 'protected draft')
                latencies.append(switch(3))
                assert content(f'{origin}/capacity/3', f"(async()=>{{window.media=new Audio('{origin}/tone.wav');media.muted=true;media.loop=true;document.body.append(media);await media.play();return !media.paused}})()")
                content(f'{origin}/capacity/3', 'window.mediaMarker="retained playback";true')
                latencies.append(switch(4))
                assert not content(f'{origin}/capacity/3', '__velunoCanSleep()'), 'hidden playback must stay protected even if WebView2 pauses it'
                command('navigate', url=origin + '/slow')
                loaded(origin + '/slow')
                content(origin + '/slow', "document.getElementById('download').click();true")
                wait(lambda snapshot: sidebar('currentState.active_downloads') == 1, 'capacity download starts')
                frame = origin.replace('127.0.0.1', 'localhost') + '/next'
                for id in (5, 6):
                    latencies.append(switch(id))
                    assert content(f'{origin}/capacity/{id}', f"new Promise(r=>document.body.append(Object.assign(document.createElement('iframe'),{{src:'{frame}?{id}',onload:()=>r(true)}})))")
                content(f'{frame}?5', "document.getElementById('note').focus();true", 'cross-origin draft')
                latencies.extend(switch(id) for id in range(7, 31))
                settled = wait(lambda snapshot: len(snapshot['tabs']) == count and sum(not tab['is_sleeping'] for tab in snapshot['tabs']) <= 12
                               and next(tab for tab in snapshot['tabs'] if tab['id'] == 6)['is_sleeping'],
                               f'{count} tabs settle to the live budget and the idle iframe tab sleeps')
                assert content(f'{frame}?5', "document.getElementById('note').value") == 'cross-origin draft'
                for id in (1, 2, 3, 4, 5):
                    assert not next(tab for tab in settled['tabs'] if tab['id'] == id)['is_sleeping'], f'protected tab {id} slept'
                assert content(f'{origin}/capacity/2', "document.getElementById('note').value") == 'protected draft'
                assert content(f'{origin}/capacity/3', 'window.mediaMarker') == 'retained playback'
                with urlopen(f'http://127.0.0.1:{content_port}/json/list', timeout=5) as response:
                    live_pages = len([page for page in json.load(response) if page['type'] == 'page'])
                assert live_pages <= 12, f'WebViews leaked: {live_pages}'
                memory = subprocess.run(['powershell', '-NoProfile', '-Command',
                                        f'$ids=@({process.pid}); $all=@(Get-CimInstance Win32_Process); '
                                        'do { $children=@($all | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids }); '
                                        '$ids+=@($children.ProcessId) } while($children.Count); '
                                        '($all | Where-Object { $_.ProcessId -in $ids } | Measure-Object WorkingSetSize -Sum).Sum'],
                                       check=True, capture_output=True, text=True, timeout=20)
                capacity.append({'tabs': count, 'live_views': live_pages, 'startup_ms': round(startup_ms),
                                 'max_switch_ms': round(max(latencies)), 'working_set_bytes': int(memory.stdout.strip())})
                Fixture.release_download.set()
                wait(lambda snapshot: sidebar('currentState.active_downloads') == 0, 'capacity download finishes')
                content(f'{origin}/capacity/3', 'media.pause();true')
                sleeper = next(tab['id'] for tab in settled['tabs'] if tab['is_sleeping'])
                switch(sleeper)
                wait(lambda snapshot: snapshot['active_tab_id'] == sleeper, 'latest selection reaches disk')
                app.stop()
                time.sleep(1)
                process = app.start([], log)
                wait(lambda _: sidebar('currentState.tabs.length') == count, 'recovered capacity sidebar ready')
                recovered = wait(lambda snapshot: snapshot['active_tab_id'] == sleeper and not active(snapshot)['is_loading'], 'capacity crash recovery')
                assert sum(not tab['is_sleeping'] for tab in recovered['tabs']) == 1
                assert next(tab for tab in recovered['tabs'] if tab['id'] == 1)['is_pinned']
                print(f'Capacity {count}: {capacity[-1]}', flush=True)
            (artifacts / 'capacity.json').write_text(json.dumps(capacity, indent=2), encoding='utf-8')
            checks.append('100/300/500-tab sessions restore lazily, bound actual WebViews, protect drafts (including cross-origin iframe drafts)/media/pins/downloads, sleep idle iframe pages, wake sleepers, and recover after forced exit')
        (artifacts / 'results.json').write_text(json.dumps({'success': True, 'checks': checks}, indent=2), encoding='utf-8')
        print('\n'.join(checks))
        print(f'Headless regression tests passed. Evidence: {artifacts}')
    finally:
        Fixture.release_download.set()
        try:
            if sys.exc_info()[0] is not None and process is not None:
                print(f'Failed app PID {process.pid}, exit code {process.poll()}', flush=True)
                print(json.dumps((artifacts / 'app.log').read_text(encoding='utf-8', errors='replace')), flush=True)
                snapshot = state()
                print(f'Saved fixture session: {json.dumps({"tabs": len(snapshot["tabs"]), "active": active(snapshot)}) if snapshot else None}', flush=True)
                (artifacts / 'failure.json').write_text(json.dumps({
                    'error': str(sys.exc_info()[1]), 'pid': process.pid, 'exit_code': process.poll(),
                    'active': active(snapshot) if snapshot else None,
                }, indent=2), encoding='utf-8')
                if not (artifacts / 'debugger.json').exists():
                    try:
                        subprocess.run(['node', str(Path(__file__).with_name('native_capture.mjs')),
                                        str(port), str(content_port), str(artifacts), '--diagnostics'], timeout=60)
                    except (OSError, subprocess.TimeoutExpired) as error:
                        print(f'Could not collect debugger evidence: {error}', flush=True)
                subprocess.run(['powershell', '-NoProfile', '-Command',
                                f'$parents = @({process.pid}); $all = @(Get-CimInstance Win32_Process); '
                                'do { $children = @($all | Where-Object { $_.ParentProcessId -in $parents -and $_.ProcessId -notin $parents }); '
                                '$parents += @($children.ProcessId) } while ($children.Count); '
                                '$all | Where-Object { $_.ProcessId -in $parents -and $_.CommandLine -notmatch " --type=" } | Select-Object ProcessId,ParentProcessId,Name,CommandLine | Format-List'],
                               timeout=15)
        finally:
            try:
                app.close()
            finally:
                server.shutdown()



if __name__ == '__main__':
    main()
