import { connect, writeDiagnostics, closeConnections } from './native_cdp.mjs';
import assert from 'node:assert/strict';

const [chromePort, contentPort, artifacts, mode] = process.argv.slice(2);
try {
  if (mode !== '--diagnostics') {
    const sidebar = await connect(page => page.title === 'Veluno', chromePort);
    const deadline = Date.now() + 10000;
    while (!await sidebar.evaluate("typeof currentState === 'object' && !!currentState?.tabs?.some(tab => tab.id === currentState.active_tab_id && !tab.is_loading)")) {
      if (Date.now() > deadline) throw Error('Capture fixture did not finish loading');
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    await sidebar.evaluate(`document.getElementById('app-menu').open = true; true`);
  }
  const targets = await writeDiagnostics([chromePort, contentPort], artifacts, null, mode !== '--diagnostics');
  if (mode !== '--diagnostics') {
    assert.ok(targets.some(target => target.title === 'Veluno' && target.screenshot), 'Sidebar capture missing');
    assert.ok(targets.some(target => target.url?.startsWith('http:') && target.screenshot), 'Fixture page capture missing');
    assert.ok(targets.every(target => !target.error), 'A WebView could not be captured; see debugger.json');
  }
} finally {
  closeConnections();
}
