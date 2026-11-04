import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect } from './native_cdp.mjs';

async function until(probe, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    try { if (await probe()) return; }
    catch (error) { if (!error.message.includes('Execution context was destroyed')) throw error; }
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw Error('Timed out: ' + label);
}

export async function checkBrowserControls(sidebar, command, origin, port, artifacts) {
  await command('create_profile', { name: 'Controls test' });
  await until(() => sidebar.evaluate('currentState.workspaces.some(w=>w.name==="Controls test"&&w.id===currentState.active_workspace_id)'), 'workspace creation');
  const workspace = await sidebar.evaluate('currentState.active_workspace_id');
  const destination = await sidebar.evaluate('currentState.workspaces.find(w=>!w.private&&w.id!==currentState.active_workspace_id).id');
  await command('new_tab', { url: origin + '/controls-a' });
  const page = await connect(target => target.url === origin + '/controls-a', port);
  await until(() => page.evaluate(`location.href===${JSON.stringify(origin + '/controls-a')} && document.readyState==="complete"`), 'controls page');
  const a = await sidebar.evaluate('currentState.active_tab_id');
  await page.evaluate(`document.body.insertAdjacentHTML('beforeend','<input id="preserved-draft" value="Keep this draft">');localStorage.setItem('controls','preserved');document.cookie='controls=preserved;path=/';true`);
  await page.evaluate(`document.head.insertAdjacentHTML('beforeend','<link rel="icon" href="${origin}/assets/favicon.svg">');true`);
  await until(()=>sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).favicon===${JSON.stringify(origin+'/assets/favicon.svg')}`),'native site favicon captured');
  await sidebar.evaluate(`window.focusAddress();const input=document.getElementById('url-input');input.value='controls';input.dispatchEvent(new Event('input'));true`);
  await until(()=>sidebar.evaluate(`document.querySelector('#address-results .address-option-icon img')?.naturalWidth>0`),'address suggestion site logo loaded');
  assert.equal(await sidebar.evaluate(`document.querySelector('#address-results .address-option-icon img').referrerPolicy`),'no-referrer');
  if(artifacts) {
    const screenshot=await sidebar.call('Page.captureScreenshot',{format:'png'});
    await writeFile(join(artifacts,'address-suggestion-logos.png'),Buffer.from(screenshot.data,'base64'));
  }
  await sidebar.evaluate(`document.getElementById('url-input').blur();true`);
  await command('new_tab', { url: origin + '/controls-b' });
  await until(() => sidebar.evaluate(`currentState.tabs.some(t=>t.url===${JSON.stringify(origin + '/controls-b')}&&t.id===currentState.active_tab_id)`), 'second controls tab');
  const b = await sidebar.evaluate('currentState.active_tab_id');
  await command('reorder_tab', { id: b, target: a, after: false });
  await until(() => sidebar.evaluate(`currentState.tabs.findIndex(t=>t.id===${b})<currentState.tabs.findIndex(t=>t.id===${a})`), 'native reorder');
  await sidebar.evaluate(`document.querySelector('.tab-main[data-tab-id="${b}"]').focus();true`);
  for (const type of ['keyDown','keyUp']) await sidebar.call('Input.dispatchKeyEvent',{type,key:'ArrowDown',windowsVirtualKeyCode:40,modifiers:9});
  await until(() => sidebar.evaluate(`currentState.tabs.findIndex(t=>t.id===${a})<currentState.tabs.findIndex(t=>t.id===${b})`), 'native keyboard reorder');
  await sidebar.evaluate(`document.getElementById('profiles').open=true;document.getElementById('rename-workspace').click();document.getElementById('profile-name').value='Research';document.getElementById('profile-form').requestSubmit();true`);
  await until(() => sidebar.evaluate(`currentState.workspaces.some(w=>w.id===${workspace}&&w.name==='Research')`), 'workspace rename');
  await command('toggle_mute', { id: a });
  await until(() => sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_muted`), 'native mute');
  await page.call('Runtime.evaluate', { expression: `window.controlsAudio=new AudioContext();window.controlsTone=controlsAudio.createOscillator();controlsTone.connect(controlsAudio.destination);controlsTone.start();controlsAudio.resume();true`, userGesture: true });
  await until(() => sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_playing_audio`), 'native playing-audio event');
  await sidebar.evaluate(`document.querySelector('.audio-toggle[data-tab-id="${a}"]').click();true`);
  await until(() => sidebar.evaluate(`!currentState.tabs.find(t=>t.id===${a}).is_muted`), 'unmute control');
  await command('toggle_mute', { id: a });
  await page.evaluate('controlsTone.stop();controlsAudio.close();true');
  await command('toggle_pin', { id: a });
  await sidebar.evaluate(`document.getElementById('profiles').open=true;document.getElementById('delete-workspace').click();document.getElementById('workspace-destination').value='${destination}';document.getElementById('confirm-delete-workspace').click();true`);
  await until(() => sidebar.evaluate(`!currentState.workspaces.some(w=>w.id===${workspace})&&currentState.tabs.find(t=>t.id===${a}).workspace_id===${destination}`), 'workspace deletion preserves tabs');
  assert.equal(await page.evaluate('document.getElementById("preserved-draft").value'), 'Keep this draft');
  assert.equal(await sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_pinned`), true);

  const otherOrigin = origin.replace('127.0.0.1', 'localhost');
  assert.notEqual(otherOrigin, origin);
  await command('new_tab', { url: otherOrigin + '/other-storage' });
  const other = await connect(target => target.url === otherOrigin + '/other-storage', port);
  await until(() => other.evaluate(`location.href===${JSON.stringify(otherOrigin + '/other-storage')} && document.readyState==="complete"`), 'other origin');
  await other.evaluate(`localStorage.setItem('controls','keep-other');document.cookie='controls=keep-other;path=/';true`);
  await command('switch_tab', { id: a });
  await until(() => sidebar.evaluate(`currentState.active_tab_id===${a}`), 'site controls target');
  await command('clear_site_data', { id: a, origin: otherOrigin });
  assert.equal(await page.evaluate('localStorage.getItem("controls")'), 'preserved', 'mismatched origin must be rejected');
  await sidebar.evaluate(`document.getElementById('security').click();true`);
  assert.equal(await sidebar.evaluate('document.getElementById("site-origin").textContent'), origin);
  if (artifacts) {
    const screenshot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(artifacts, 'site-controls.png'), Buffer.from(screenshot.data, 'base64'));
  }
  await sidebar.evaluate(`document.getElementById('clear-site').click();document.getElementById('cancel-clear-site').click();true`);
  assert.equal(await page.evaluate('localStorage.getItem("controls")'), 'preserved');
  await sidebar.evaluate(`document.getElementById('clear-site').click();document.getElementById('confirm-clear-site').click();true`);
  await until(() => sidebar.evaluate(`currentState.update_status===${JSON.stringify('Site data cleared for ' + origin + '.')}`), 'native clear completion');
  await until(() => page.evaluate('document.readyState==="complete"&&localStorage.getItem("controls")===null'), 'origin storage cleared and reloaded');
  assert.equal(await page.evaluate('document.cookie.includes("controls=")'), false);
  assert.equal(await other.evaluate('localStorage.getItem("controls")'), 'keep-other');
  assert.equal(await other.evaluate('document.cookie.includes("controls=keep-other")'), true);
  assert.equal(await sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_muted`), true, 'mute survives reload');
  await command('switch_tab',{id:b});
  await until(()=>sidebar.evaluate(`currentState.active_tab_id===${b}`),'leave muted tab');
  await command('toggle_sleep',{id:a});
  await until(()=>sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_sleeping`),'sleep muted tab');
  await command('switch_tab',{id:a});
  const awakened=await connect(target=>target.url===origin+'/controls-a',port);
  await until(()=>awakened.evaluate(`location.href===${JSON.stringify(origin+'/controls-a')}&&document.readyState==="complete"`),'wake muted tab');
  await awakened.call('Runtime.evaluate',{expression:'window.recreatedAudio=new AudioContext();window.recreatedTone=recreatedAudio.createOscillator();recreatedTone.connect(recreatedAudio.destination);recreatedTone.start();recreatedAudio.resume();true',userGesture:true});
  await until(()=>sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_playing_audio`),'audio event from recreated view');
  assert.equal(await sidebar.evaluate(`currentState.tabs.find(t=>t.id===${a}).is_muted`),true,'native mute is reapplied before loading a recreated page');
  await awakened.evaluate('recreatedTone.stop();recreatedAudio.close();true');
  console.log('Native tab ordering, workspace rename/delete with live draft retention, audio/mute events, and origin-scoped cookie/storage clearing passed.');
  await checkGlance(sidebar, awakened, origin, port, artifacts);
  await command('set_setting', { key: 'accent', value: 'glacier' });
  await until(() => sidebar.evaluate(`getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()==='#8ecbff'`), 'accent applies live to the sidebar');
  await command('set_setting', { key: 'accent', value: 'neutral' });
  await until(() => sidebar.evaluate(`getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()==='#dedee3'`), 'accent resets');
  console.log('Accent color changes apply live to the sidebar.');
  await checkSplitView(sidebar, command, awakened, a, b, origin, port, artifacts);
  const space = await sidebar.evaluate('currentState.active_workspace_id');
  await sidebar.evaluate(`document.getElementById('profiles').open=true;document.querySelector('#workspace-emojis button[aria-label="Use \u{1F4DA} as the workspace icon"]').click();true`);
  await until(() => sidebar.evaluate(`currentState.workspaces.find(w=>w.id===${space}).emoji==='\u{1F4DA}'&&document.getElementById('workspace-emoji').textContent==='\u{1F4DA}'&&!document.getElementById('workspace-emoji').hidden`), 'workspace emoji shows in the heading');
  assert.ok(await sidebar.evaluate(`document.getElementById('profile-current').textContent.startsWith('\u{1F4DA} ')`));
  await sidebar.evaluate(`document.querySelector('#workspace-emojis button[aria-label="No workspace icon"]').click();document.getElementById('profiles').open=false;true`);
  await until(() => sidebar.evaluate(`!currentState.workspaces.find(w=>w.id===${space}).emoji&&document.getElementById('workspace-emoji').hidden`), 'workspace emoji clears');
  console.log('Workspace emoji icons can be chosen from the workspace menu and cleared.');
  await checkSidePanel(sidebar, command, a, b, origin, port);
}

async function checkSidePanel(sidebar, command, a, b, origin, port) {
  await command('switch_tab', { id: b });
  await until(() => sidebar.evaluate(`currentState.active_tab_id===${b}`), 'side panel test tab');
  const page = await connect(target => target.url === origin + '/controls-b', port);
  const before = new Set((await targets(port)).map(target => target.id));
  const fullWidth = await page.evaluate('innerWidth');
  await command('open_in_side_panel', { id: a });
  const panel = await connect(target => target.url === origin + '/controls-a' && !before.has(target.id), port);
  await until(() => panel.evaluate('document.readyState==="complete"&&document.visibilityState==="visible"'), 'side panel loads beside the page');
  await until(async () => await page.evaluate('innerWidth') < fullWidth - 100, 'the page narrows for the side panel');
  assert.equal(await sidebar.evaluate(`document.getElementById('app-side-panel-label').textContent`), 'Hide side panel');
  assert.equal(await sidebar.evaluate('currentState.active_tab_id'), b, 'docking a site keeps the current tab');
  await panel.evaluate('window.panelMarker=1;true');
  await command('toggle_side_panel');
  await until(async () => !(await panel.evaluate('document.visibilityState==="visible"')) && await page.evaluate('innerWidth') === fullWidth, 'hiding the side panel restores the page width');
  await command('toggle_side_panel');
  await until(() => panel.evaluate('document.visibilityState==="visible"'), 'side panel shows again');
  assert.equal(await panel.evaluate('window.panelMarker'), 1, 'the side panel keeps its page while hidden');
  await command('toggle_side_panel');
  await until(async () => await page.evaluate('innerWidth') === fullWidth, 'side panel hidden for later checks');
  console.log('The side web panel docks a site beside the current tab, narrows the page, and hides without reloading.');
}

async function checkSplitView(sidebar, command, left, a, b, origin, port, artifacts) {
  const other = await sidebar.evaluate('currentState.active_tab_id');
  await command('switch_tab', { id: a });
  await command('split_tab', { id: b });
  await until(() => sidebar.evaluate(`JSON.stringify(currentState.split)===${JSON.stringify(JSON.stringify([a, b]))}`), 'split pair');
  const right = await connect(target => target.url === origin + '/controls-b', port);
  const visible = view => view.evaluate('document.visibilityState==="visible"');
  await until(async () => await visible(left) && await visible(right), 'both split panes are visible');
  const [leftWidth, rightWidth, sidebarWidth] = await Promise.all([left.evaluate('innerWidth'), right.evaluate('innerWidth'), sidebar.evaluate('innerWidth')]);
  assert.ok(Math.abs(leftWidth - rightWidth) <= 1, `panes share the width: ${leftWidth} vs ${rightWidth}`);
  assert.ok(leftWidth < 300, `each pane is half the content area: ${leftWidth} beside a ${sidebarWidth}px sidebar`);
  assert.equal(await sidebar.evaluate(`document.querySelectorAll('.tab.split').length`), 2, 'sidebar marks both split tabs');
  if (artifacts) {
    const screenshot = await sidebar.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(artifacts, 'split-view-sidebar.png'), Buffer.from(screenshot.data, 'base64'));
  }
  await command('switch_tab', { id: other });
  await until(async () => !(await visible(left)) && !(await visible(right)), 'leaving the split hides both panes');
  await command('switch_tab', { id: b });
  await until(async () => await visible(left) && await visible(right), 'selecting either split tab shows both');
  await command('unsplit');
  await until(async () => !(await visible(left)) && await visible(right), 'closing split view keeps only the active tab');
  assert.ok(await right.evaluate('innerWidth') > leftWidth + 100, 'the active tab regains the full width');
  console.log('Split View shows two tabs side by side, follows tab selection, and closes back to one tab.');
}

async function targets(port) {
  return (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
}

async function checkGlance(sidebar, page, origin, port, artifacts) {
  const glanceUrl = origin + '/glance-target';
  const tabsBefore = await sidebar.evaluate('currentState.tabs.length');
  const activeBefore = await sidebar.evaluate('currentState.active_tab_id');
  const altClick = async () => {
    const { x, y } = await page.evaluate(`(() => {
      document.getElementById('glance-link')?.remove();
      document.body.insertAdjacentHTML('afterbegin','<a id="glance-link" href="/glance-target" style="display:block;padding:20px">Glance</a>');
      const rect = document.getElementById('glance-link').getBoundingClientRect();
      return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
    })()`);
    for (const type of ['mousePressed', 'mouseReleased']) await page.call('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, modifiers: 1 });
  };
  await altClick();
  const glance = await connect(target => target.url === glanceUrl, port);
  await until(() => glance.evaluate('document.readyState==="complete"'), 'Glance preview loads');
  assert.equal(await page.evaluate('location.pathname'), '/controls-a', 'Alt+Click leaves the current tab in place');
  assert.equal(await sidebar.evaluate('currentState.tabs.length'), tabsBefore, 'Glance does not create a tab');
  assert.equal(await sidebar.evaluate('currentState.active_tab_id'), activeBefore);
  if (artifacts) {
    const screenshot = await glance.call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(artifacts, 'glance.png'), Buffer.from(screenshot.data, 'base64'));
  }
  for (const type of ['keyDown', 'keyUp']) await glance.call('Input.dispatchKeyEvent', { type, key: 'Escape', windowsVirtualKeyCode: 27 }, { allowTargetClose: true });
  await until(async () => !(await targets(port)).some(target => target.url === glanceUrl), 'Escape closes Glance');
  await altClick();
  await connect(target => target.url === glanceUrl, port);
  await sidebar.evaluate(`window.ipc.postMessage(JSON.stringify({action:'expand_popup',url:${JSON.stringify(glanceUrl)}}));true`);
  await until(() => sidebar.evaluate(`currentState.tabs.some(t=>t.url===${JSON.stringify(glanceUrl)}&&t.id===currentState.active_tab_id)`), 'Open as tab promotes Glance');
  assert.equal(await sidebar.evaluate('currentState.tabs.length'), tabsBefore + 1);
  console.log('Glance: Alt+Click previews a link without leaving the tab, Escape dismisses it, and Open as tab promotes it.');
}
