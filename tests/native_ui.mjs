import { checkBrowserControls } from './native_controls.mjs';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect as connectView, closeConnections, writeDiagnostics } from './native_cdp.mjs';

const [chromePort, contentPort, origin, artifacts] = process.argv.slice(2);
const connect = (matches, port = chromePort) => connectView(matches, port);
async function until(probe, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    const value = await probe();
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}
async function press(view, key, modifiers = 0) {
  const params = { key, modifiers, windowsVirtualKeyCode: key === 'Escape' ? 27 : key === 'Enter' ? 13 : key.toUpperCase().charCodeAt(0) };
  if (key === 'Enter') params.text = '\r';
  await view.call('Input.dispatchKeyEvent', { type: 'keyDown', ...params });
  await view.call('Input.dispatchKeyEvent', { type: 'keyUp', ...params });
}

try {
  const sidebar = await connect(page => page.title === 'Veluno');
  await sidebar.call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'no-preference' }] });
  const menuOpacity = await sidebar.evaluate(`(() => {
    const menu = document.getElementById('app-menu');
    menu.open = true;
    const panel = menu.querySelector('.menu');
    const animation = panel.getAnimations()[0];
    const opacity = [0, 0.25, 0.75, 1].map(progress => {
      if (animation) { animation.pause(); animation.currentTime = progress * animation.effect.getTiming().duration; }
      return getComputedStyle(panel).opacity;
    });
    animation?.finish();
    menu.open = false;
    return opacity;
  })()`);
  assert.deepEqual(menuOpacity, ['1', '1', '1', '1'], 'App menu must stay opaque throughout its opening animation');
  await sidebar.call('Emulation.setEmulatedMedia', { features: [] });
  const command = (action, payload = {}) => sidebar.evaluate(`window.ipc.postMessage(${JSON.stringify(JSON.stringify({ action, ...payload }))});true`);
  const palette = await connect(page => page.title.includes('New tab search'));
  const originalTab = await sidebar.evaluate('currentState.active_tab_id');
  await command('new_tab', { url: origin + '/search-match' });
  const searchMatch = await connect(page => page.url === origin + '/search-match', contentPort);
  await until(() => searchMatch.evaluate(`location.href === ${JSON.stringify(origin + '/search-match')} && document.readyState === "complete" && document.title === "Veluno fixture"`), 'search fixture loaded');
  await searchMatch.evaluate('document.title = "YouTube search fixture";true');
  await until(() => sidebar.evaluate('currentState.tabs.some(tab=>tab.title === "YouTube search fixture")'), 'search fixture title');
  const matchId = await sidebar.evaluate('currentState.active_tab_id');
  await command('switch_tab', { id: originalTab });
  await until(() => sidebar.evaluate(`currentState.active_tab_id === ${originalTab}`), 'return to the original tab');
  await command('new_tab');
  await until(() => palette.evaluate(`!input.readOnly && document.activeElement === input && state.tabs.some(tab=>tab.id === ${matchId} && tab.title === "YouTube search fixture")`), 'palette ready for search ranking');
  await palette.evaluate('input.value="you";input.dispatchEvent(new Event("input"));true');
  assert.equal(await palette.evaluate('document.querySelector(".option[aria-selected=true] .option-title").textContent'), 'YouTube search fixture');
  await press(palette, 'Enter');
  await until(() => sidebar.evaluate(`currentState.active_tab_id === ${matchId}`), 'Enter switches to the obvious tab');
  await command('close_tab', { id: matchId });
  await until(() => sidebar.evaluate(`!currentState.tabs.some(tab=>tab.id === ${matchId})`), 'search fixture tab closes');
  console.log('Native search ranking selects the obvious tab and Enter switches to it without a search.');
  const count = await sidebar.evaluate('currentState.tabs.length');
  await command('new_tab', { url: origin + '/keyboard' });
  const content = await connect(page => page.url === origin + '/keyboard', contentPort);
  await until(() => content.evaluate(`location.href === ${JSON.stringify(origin + '/keyboard')} && document.readyState === 'complete' && !!window.Veluno`), 'keyboard fixture');
  await sidebar.evaluate(`(()=>{const update=window.updateAddressHistory;window.addressSuggestionCalls=0;window.updateAddressHistory=entries=>{window.addressSuggestionCalls++;update(entries)}})()`);
  await content.evaluate('window.chrome.webview.postMessage(JSON.stringify({action:"address_suggestions"}));true');
  await new Promise(resolve => setTimeout(resolve, 200));
  assert.equal(await sidebar.evaluate('window.addressSuggestionCalls'), 0, 'Remote pages cannot request local address history');
  await content.evaluate('window.pageMarker = "kept"');
  await press(content, 't', 2);
  await until(() => palette.evaluate('document.activeElement.id === "new-tab-input" && !input.readOnly'), 'Ctrl+T opens palette');
  assert.equal(await sidebar.evaluate('currentState.tabs.length'), count + 1);
  await palette.evaluate('input.value = "draft query"');
  await press(palette, 'Escape');
  await until(() => palette.evaluate('input.readOnly'), 'Escape dismisses palette');
  assert.equal(await content.evaluate('window.pageMarker'), 'kept');
  await sidebar.evaluate('document.getElementById("btn-add-tab").click()');
  await until(() => palette.evaluate('!input.readOnly && input.value === ""'), 'sidebar opens fresh palette');
  await palette.evaluate(`input.value=${JSON.stringify(origin + '/submitted')};input.dispatchEvent(new Event('input'))`);
  await press(palette, 'Enter');
  await until(() => sidebar.evaluate(`currentState.tabs.length === ${count + 2}`), 'Enter creates one tab');
  const submitted = await connect(page => page.url === origin + '/submitted', contentPort);
  await until(() => submitted.evaluate(`location.href === ${JSON.stringify(origin + '/submitted')} && document.readyState === 'complete' && !!window.Veluno`), 'submitted page ready for keyboard input');
  await submitted.call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'w', modifiers: 2, windowsVirtualKeyCode: 87 }, { allowTargetClose: true });
  await until(() => sidebar.evaluate(`currentState.tabs.length === ${count + 1}`), 'Ctrl+W closes tab');
  await content.call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'w', modifiers: 2, windowsVirtualKeyCode: 87 });
  await press(content, 'T', 10);
  await until(() => sidebar.evaluate(`currentState.tabs.length === ${count + 2}`), 'Ctrl+Shift+T restores tab');
  const restored = await connect(page => page.url === origin + '/submitted', contentPort);
  await until(() => restored.evaluate(`location.href === ${JSON.stringify(origin + '/submitted')} && document.readyState === 'complete' && !!window.Veluno`), 'restored page ready for keyboard input');
  await press(restored, 'B', 10);
  await until(() => sidebar.evaluate('currentState.sidebar_collapsed'), 'Ctrl+Shift+B collapses sidebar');
  await press(restored, 'l', 2);
  await until(() => sidebar.evaluate('!currentState.sidebar_collapsed && document.activeElement.id === "url-input"'), 'Ctrl+L expands and selects address');
  if (await sidebar.evaluate('currentState.workspaces.length < 2')) await command('create_profile', { name: 'School' });
  await until(() => sidebar.evaluate('currentState.workspaces.length >= 2'), 'Profile exists');
  await press(restored, '1', 1);
  await until(() => sidebar.evaluate('currentState.active_workspace_id === 1'), 'Alt+1 switches profile');
  await press(restored, '2', 1);
  await until(() => sidebar.evaluate('currentState.active_workspace_id === 2'), 'Alt+2 switches profile');
  await press(sidebar, '1', 1);
  await until(() => sidebar.evaluate('currentState.active_workspace_id === 1'), 'Alt+1 returns to profile');
  const pins = await sidebar.evaluate('document.querySelectorAll("#pinned-tabs .tab").length');
  const pinChange = await sidebar.evaluate('currentState.tabs.find(tab => tab.id === currentState.active_tab_id).is_pinned ? -1 : 1');
  await sidebar.evaluate('document.getElementById("btn-pin-tab").click()');
  await until(() => sidebar.evaluate(`document.querySelectorAll('#pinned-tabs .tab').length === ${pins + pinChange}`), 'toggle active tab pin');
  await sidebar.evaluate('document.getElementById("btn-pin-tab").click()');
  await palette.call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
  await press(sidebar, 't', 2);
  await until(() => palette.evaluate('!input.readOnly'), 'reduced-motion palette');
  assert.equal(await palette.evaluate('document.getAnimations().filter(animation => animation.id.startsWith("palette-") && animation.playState === "running").length'), 0);
  await press(palette, 'Escape');
  await until(() => palette.evaluate('input.readOnly'), 'reduced-motion cancel');
  assert.equal(await restored.evaluate('typeof window.ipc'), 'undefined', 'pages keep their own ipc global, as Google Keep needs');
  const beforePageIpc = await sidebar.evaluate('currentState.tabs.length');
  await restored.evaluate('Veluno.send("new_tab"); Veluno.send("duplicate_tab", { id: 1 }); Veluno.send("move_tab_to_workspace", { id: 1, workspace_id: 2 }); Veluno.send("expand_popup", { url: "https://example.com" })');
  await new Promise(resolve => setTimeout(resolve, 200));
  assert.equal(await sidebar.evaluate('currentState.tabs.length'), beforePageIpc, 'untrusted page IPC cannot create or mutate tabs');
  const popupBar = await connect(page => page.title === 'Veluno Popup');
  await restored.evaluate(`window.authDone = false; window.open(${JSON.stringify(origin + '/oauth/login')}, 'oauth', 'width=480,height=620'); true`);
  const oauthPopup = await connect(page => page.url === origin + '/oauth/login', contentPort);
  await until(() => oauthPopup.evaluate('document.readyState === "complete" && !!window.opener'), 'OAuth popup has live window.opener');
  await until(() => popupBar.evaluate('document.getElementById("address").textContent.includes("/oauth/login")'), 'Popup URL bar displays login URL');
  assert.equal(await sidebar.evaluate('currentState.tabs.length'), beforePageIpc, 'OAuth popup does not clutter tab list');
  await oauthPopup.evaluate('window.opener.authDone = true; window.close()');
  await until(() => restored.evaluate('window.authDone === true'), 'OAuth popup communicates with opener and closes');
  console.log('Native keyboard, palette open/cancel/submit, page retention, restoration, compact address focus, workspaces, pinning, reduced motion, OAuth popup, and IPC guard passed.');

  await command('new_tab');
  await until(() => palette.evaluate('!input.readOnly'), 'open palette for pointer regression');
  await palette.evaluate(`window.openNewTab({tabs:[],shortcuts:[{title:'Facebook',url:'${origin}/facebook'},{title:'X',url:'${origin}/x'}]},[])`);
  const xRow = await palette.evaluate('(()=>{const r=document.querySelectorAll(".option")[1].getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()');
  await palette.call('Input.dispatchMouseEvent', { type: 'mouseMoved', ...xRow });
  assert.equal(await palette.evaluate('document.querySelector(".option[aria-selected=true] .option-title").textContent'), 'X');
  await press(palette, 'Enter');
  await until(() => sidebar.evaluate(`currentState.tabs.find(tab=>tab.id===currentState.active_tab_id)?.url === '${origin}/x'`), 'Enter opens pointer-selected X result');
  console.log('Native pointer movement selects X and Enter opens the same result.');

  const permission = await connect(page => page.title === 'Veluno — Site permission');
  for (const [site, allow] of [[origin, false], [origin.replace('127.0.0.1', 'localhost'), true]]) {
    await command('new_tab', { url: site + '/permission' });
    const requester = await connect(page => page.url === site + '/permission', contentPort);
    await until(() => requester.evaluate(`location.href === ${JSON.stringify(site + '/permission')} && document.readyState === "complete"`), 'permission fixture ready');
    await requester.call('Runtime.evaluate', { expression: 'window.permissionResult="pending";Notification.requestPermission().then(result=>window.permissionResult=result);true', userGesture: true });
    await until(() => permission.evaluate(`current?.origin === ${JSON.stringify(site)} && current.kind === 'notifications'`), 'notification permission prompt');
    assert.equal(await permission.evaluate('document.getElementById("title").textContent'), 'Show notifications?');
    assert.equal(await permission.evaluate('document.getElementById("origin").textContent'), site);
    assert.equal(await permission.evaluate('document.activeElement.id'), 'block');
    if (artifacts && !allow) {
      const screenshot = await permission.call('Page.captureScreenshot', { format: 'png' });
      await writeFile(join(artifacts, 'permission.png'), Buffer.from(screenshot.data, 'base64'));
    }
    await permission.evaluate(`document.getElementById('${allow ? 'allow' : 'block'}').click()`);
    await until(() => requester.evaluate(`window.permissionResult === '${allow ? 'granted' : 'denied'}'`), 'native permission decision applied');
    await requester.call('Page.navigate', { url: site + '/permission-repeat' });
    await until(() => requester.evaluate('document.readyState === "complete" && location.pathname === "/permission-repeat"'), 'repeat fixture ready');
    await requester.call('Runtime.evaluate', { expression: 'window.permissionResult="pending";Notification.requestPermission().then(result=>window.permissionResult=result);true', userGesture: true });
    await until(() => requester.evaluate(`window.permissionResult === '${allow ? 'granted' : 'denied'}'`), 'saved permission decision applied without prompt');
    assert.equal(await permission.evaluate('current'), null, 'remembered permission does not prompt again');
  }
  console.log('Native notification permission prompts show the requesting origin and apply Block and Allow, persisting decisions across visits.');

  await command('new_tab', { url: origin + '/location' });
  const locator = await connect(page => page.url === origin + '/location', contentPort);
  await until(() => locator.evaluate(`location.href === ${JSON.stringify(origin + '/location')} && document.readyState === "complete"`), 'location fixture ready');
  await locator.call('Runtime.evaluate', { expression: 'navigator.geolocation.getCurrentPosition(()=>{},()=>{});true', userGesture: true });
  await until(() => permission.evaluate('current?.kind === "location"'), 'location prompt');
  const staleToken = await permission.evaluate('current.token');
  await locator.evaluate(`window.chrome.webview.postMessage(JSON.stringify({token:${staleToken},allow:true}));true`);
  assert.equal(await permission.evaluate('current.token'), staleToken, 'page IPC cannot answer a browser permission prompt');
  assert.notEqual(await locator.evaluate('navigator.permissions.query({name:"geolocation"}).then(result=>result.state)'), 'granted');
  await command('navigate', { url: origin + '/navigated' });
  await until(() => permission.evaluate('current === null'), 'navigation dismisses pending permission');
  await permission.evaluate(`window.ipc.postMessage(JSON.stringify({token:${staleToken},allow:true}));true`);
  assert.equal(await permission.evaluate('current'), null);
  await until(() => locator.evaluate(`location.href === '${origin}/navigated' && document.readyState === 'complete'`), 'navigation completes');
  assert.notEqual(await locator.evaluate('navigator.permissions.query({name:"geolocation"}).then(result=>result.state)'), 'granted', 'stale responses cannot grant a permission after navigation');
  console.log('Navigation dismisses a pending permission; stale responses cannot grant it.');

  await command('new_tab', { url: origin + '/tests/ui.html' });
  const fixture = await connect(page => page.url === origin + '/tests/ui.html', contentPort);
  await fixture.call('Emulation.setFocusEmulationEnabled', { enabled: true });
  for (const [width, height, motion] of [[1380, 860, 'no-preference'], [640, 480, 'reduce']]) {
    await fixture.call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await fixture.call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: motion }] });
    await fixture.call('Page.navigate', { url: `${origin}/tests/ui.html?size=${width}` });
    const result = await until(async () => {
      try {
        return await fixture.evaluate(`location.search === '?size=${width}' && (() => { const text=document.getElementById('results')?.textContent;return text?.includes('passed') || text?.startsWith('FAIL:') ? text : null })()`);
      } catch (error) {
        if (!error.message.includes('Execution context was destroyed')) throw error;
        return null;
      }
    }, 'DOM fixture completion');
    assert.ok(result.endsWith('checks passed'), `${width}x${height}, ${motion}: ${result}`);
    const checkCount = await fixture.evaluate('window.checkResults.length');
    assert.equal(result, `${checkCount} checks passed`, `${width}x${height}, ${motion}`);
    console.log(`${result}: ${width}x${height}, ${motion}`);
  }
  await command('open_settings');
  const settings = await connect(page => page.title === 'Settings', contentPort);
  const isDefault = await sidebar.evaluate('currentState.is_default_browser');
  await until(() => settings.evaluate('typeof window.setDefaultBrowser === "function"'), 'default browser settings ready');
  assert.equal(await settings.evaluate('document.getElementById("set-default-browser").hidden'), isDefault);
  for (const selected of [true, false]) {
    assert.deepEqual(await settings.evaluate(`(() => {
      window.setDefaultBrowser(${selected});
      return { hidden: document.getElementById('set-default-browser').hidden, selected: document.getElementById('default-browser-status').textContent === 'Veluno is your default browser.' };
    })()`), { hidden: selected, selected });
  }
  await settings.call('Emulation.setDeviceMetricsOverride', { width: 640, height: 480, deviceScaleFactor: 1, mobile: false });
  assert.equal(await settings.evaluate('[...document.querySelectorAll(".nav-item")].every(button=>button.getAttribute("aria-label")&&getComputedStyle(button.querySelector("span")).display!=="none")'), true, 'narrow Settings categories retain visible and accessible names');
  assert.equal(await settings.evaluate('document.documentElement.scrollWidth<=innerWidth'), true, 'Settings fits the minimum viewport');
  assert.equal(await settings.evaluate(`document.querySelector('[data-section="search"] .note').textContent.includes('bookmarks, history, and shortcuts')`), true);
  assert.equal(await settings.evaluate(`(() => {
    let requested=0; const original=Veluno.send,originalConfirm=window.confirm;
    try { Veluno.send=()=>requested++; window.confirm=()=>false; document.getElementById('clear-range').value='all'; document.getElementById('clear-data-button').click(); return requested===0; }
    finally { Veluno.send=original; window.confirm=originalConfirm; }
  })()`), true, 'cancelling the all-time Clear browsing data confirmation keeps data');
  assert.deepEqual(await settings.evaluate(`(() => {
    let sent; const original=Veluno.send;
    try {
      Veluno.send=(action,payload)=>sent={action,...payload};
      document.getElementById('clear-range').value='hour';
      for (const box of document.querySelectorAll('[data-kind]')) box.checked=box.dataset.kind==='cache';
      document.getElementById('clear-data-button').click();
      window.dataCleared(true);
      return sent;
    } finally { Veluno.send=original; }
  })()`), { action: 'clear_browsing_data', range: 'hour', kinds: { history: false, site_data: false, cache: true, passwords: false } });
  assert.equal(await settings.evaluate(`(() => { window.showSection('privacy'); return document.getElementById('title').textContent; })()`), 'Privacy and security');
  assert.equal(await settings.evaluate(`document.getElementById('tracking').querySelectorAll('[aria-checked="true"]').length`), 1, 'one tracking prevention level is selected');
  await settings.evaluate(`window.scrollTo(0, 0)`);
  await writeFile(join(artifacts, 'settings-privacy.png'), Buffer.from((await settings.call('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await settings.evaluate(`document.getElementById('clear-data').scrollIntoView()`);
  await writeFile(join(artifacts, 'settings-clear-data.png'), Buffer.from((await settings.call('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await settings.evaluate(`window.showSection('general')`);
  console.log('Default browser actions and settings reflect the detected Windows association and both UI states.');
  await command('open_history');
  const history = await connect(page => page.title === 'History and bookmarks', contentPort);
  await history.evaluate(`(() => {
    entries=[{url:'${origin}/history-icon',title:'History with site icon',time:Date.now()}];
    bookmarks=[{url:'${origin}/bookmark-icon',title:'Bookmark with site icon'}];
    data.shortcuts=[{url:'${origin}/',favicon:'${origin}/assets/favicon.svg'}];
    render();
  })()`);
  await until(() => history.evaluate('[...document.querySelectorAll(".site-icon img")].length === 2 && [...document.querySelectorAll(".site-icon img")].every(img=>img.complete && img.naturalWidth>0 && getComputedStyle(img).opacity==="1")'), 'history and bookmark favicons load');
  assert.equal(await history.evaluate('[...document.querySelectorAll(".site-icon img")].every(img=>img.referrerPolicy==="no-referrer" && getComputedStyle(img).width==="16px" && getComputedStyle(img).height==="16px")'), true);
  if (artifacts) {
    const screenshot = await history.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(artifacts, 'history-icons.png'), Buffer.from(screenshot.data, 'base64'));
  }
  assert.deepEqual(await history.evaluate(`(() => {
    render();
    const icon=document.querySelector('#list .site-icon');
    const img=icon.querySelector('img');
    img.dispatchEvent(new Event('error'));
    const retried=img.src === '${origin}/favicon.ico';
    img.dispatchEvent(new Event('error'));
    return { retried, fallback: !icon.querySelector('img') && icon.querySelector('svg').style.visibility !== 'hidden' };
  })()`), { retried: true, fallback: true });
  assert.equal(await history.evaluate(`(() => {
    data.shortcuts[0].favicon='javascript:alert(1)';render();
    return document.querySelector('#list .site-icon img').src === '${origin}/favicon.ico';
  })()`), true);
  console.log('History and bookmarks show saved site icons, retry the site favicon, and keep a globe fallback for missing or invalid images.');
  await command('open_settings', { section: 'privacy' });
  await until(() => settings.evaluate(`document.getElementById('title').textContent === 'Privacy and security'`), 'Ctrl+Shift+Delete opens Settings at Privacy');
  await command('new_tab', { url: origin + '/reader-article' });
  const article = await connect(page => page.url === origin + '/reader-article', contentPort);
  await until(() => article.evaluate(`location.href === ${JSON.stringify(origin + '/reader-article')} && document.readyState === 'complete'`), 'reader fixture loaded');
  await article.evaluate(`(() => {
    const paragraph = 'Reader view keeps the article, drops the clutter, and lets long text breathe on a calm page, which matters for anyone reading at length. ';
    document.body.innerHTML = '<nav><a href="/">Home</a><a href="/about">About</a></nav><article><h1>Reader fixture</h1>'
      + Array.from({ length: 8 }, () => '<p>' + paragraph.repeat(3) + '<script>window.injected=true<\\/script></p>').join('')
      + '<p><a href="javascript:alert(1)">bad link</a></p></article><aside>Sidebar promo</aside>';
    return true;
  })()`);
  await command('toggle_reader');
  await until(() => article.evaluate(`!!document.getElementById('__veluno-reader') && document.documentElement.style.overflow === 'hidden'`), 'reader view opens over the page');
  await writeFile(join(artifacts, 'reader-view.png'), Buffer.from((await article.call('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  assert.equal(await article.evaluate(`document.querySelector('nav') !== null && document.querySelector('article p') !== null`), true, 'reader view leaves the page DOM in place');
  await command('toggle_reader');
  await until(() => article.evaluate(`!document.getElementById('__veluno-reader') && document.documentElement.style.overflow === ''`), 'reader view closes and restores scrolling');
  await article.evaluate(`document.body.innerHTML = '<p>Too short.</p>'; true`);
  await command('toggle_reader');
  await until(() => sidebar.evaluate(`(currentState.update_status || '').startsWith("Reader view isn't available")`), 'short pages report reader view is unavailable');
  assert.equal(await article.evaluate(`document.getElementById('__veluno-reader')`), null);
  console.log('Settings opens at Privacy, and reader view opens over an article, closes cleanly, and declines short pages.');
  await article.evaluate(`localStorage.setItem('veluno-clear', '1'); true`);
  await command('set_setting', { key: 'tracking_prevention', value: 'strict' });
  await command('set_setting', { key: 'save_passwords', value: true });
  assert.equal(await sidebar.evaluate(`(currentState.update_status || '').startsWith('This setting needs')`), false, 'privacy settings apply to live tabs');
  await command('clear_browsing_data', { range: 'hour', kinds: { site_data: true, cache: true } });
  await until(() => sidebar.evaluate(`currentState.update_status === 'Browsing data from the last hour was cleared.'`), 'Clear browsing data reports success');
  await article.evaluate('location.reload(); true');
  await until(() => article.evaluate(`document.readyState === 'complete' && localStorage.getItem('veluno-clear') === null`), 'cleared site storage stays cleared');
  console.log('Privacy settings apply without errors and Clear browsing data removes recent site storage.');
  await checkBrowserControls(sidebar, command, origin, contentPort, artifacts);
} catch (error) {
  if (artifacts) {
    try { await writeDiagnostics([chromePort, contentPort], artifacts, error); }
    catch (failure) { console.error('Could not save native diagnostics:', failure); }
  }
  throw error;
} finally {
  closeConnections();
}
