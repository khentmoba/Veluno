import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect, closeConnections } from './native_cdp.mjs';

const [chromePort, contentPort, origin, artifacts] = process.argv.slice(2);

try {
  const sidebar = await connect(page => page.title === 'Veluno', chromePort);
  const deadline = Date.now() + 15000;
  let content;
  while (!content && Date.now() < deadline) {
    try {
      const pages = await (await fetch(`http://127.0.0.1:${contentPort}/json/list`)).json();
      for (const page of pages.filter(p => p.type === 'page')) {
        const candidate = await connect(t => t.id === page.id, contentPort);
        content = candidate;
        break;
      }
    } catch {
      // Port might still be starting up
    }
    if (!content) await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.ok(content, 'Content page target missing');

  // Wait for initial layout to settle
  while (!await sidebar.evaluate('currentState.tabs.length')) {
    await new Promise(resolve => setTimeout(resolve, 20));
  }

  // Ensure sidebar starts expanded (always-on)
  if (await sidebar.evaluate('currentState.sidebar_collapsed')) {
    await sidebar.evaluate("window.ipc.postMessage(JSON.stringify({action:'toggle_sidebar'}))");
    await new Promise(resolve => setTimeout(resolve, 500));
  }

  const initialWidth = await content.evaluate('innerWidth');
  assert.ok(initialWidth > 0, `Valid initial page width: ${initialWidth}`);

  // 1. Collapse sidebar: site width should smoothly expand
  await content.evaluate(`(() => {
    window.widths = [];
    window.sampler = setInterval(() => window.widths.push(innerWidth), 10);
    return true;
  })()`);

  await sidebar.evaluate("window.ipc.postMessage(JSON.stringify({action:'toggle_sidebar'}))");
  await new Promise(resolve => setTimeout(resolve, 500));

  const collapseSample = await content.evaluate(`(() => {
    clearInterval(window.sampler);
    return {
      widths: window.widths,
      finalWidth: innerWidth
    };
  })()`);

  const uniqueCollapseWidths = new Set(collapseSample.widths);
  const collapsedWidth = collapseSample.finalWidth;

  assert.ok(
    collapsedWidth > initialWidth + 100,
    `Page should expand when sidebar collapses (from ${initialWidth} to ${collapsedWidth})`
  );
  assert.ok(
    uniqueCollapseWidths.size >= 3,
    `Collapse transition must produce intermediate frame widths (got ${uniqueCollapseWidths.size} unique widths: ${Array.from(uniqueCollapseWidths).join(', ')})`
  );

  // 2. Expand sidebar: site width should smoothly shrink back
  await content.evaluate(`(() => {
    window.widths = [];
    window.sampler = setInterval(() => window.widths.push(innerWidth), 10);
    return true;
  })()`);

  await sidebar.evaluate("window.ipc.postMessage(JSON.stringify({action:'toggle_sidebar'}))");
  await new Promise(resolve => setTimeout(resolve, 500));

  const expandSample = await content.evaluate(`(() => {
    clearInterval(window.sampler);
    return {
      widths: window.widths,
      finalWidth: innerWidth
    };
  })()`);

  const uniqueExpandWidths = new Set(expandSample.widths);
  const restoredWidth = expandSample.finalWidth;

  assert.equal(
    restoredWidth,
    initialWidth,
    `Page should return to initial docked width (got ${restoredWidth}, expected ${initialWidth})`
  );
  assert.ok(
    uniqueExpandWidths.size >= 3,
    `Expand transition must produce intermediate frame widths (got ${uniqueExpandWidths.size} unique widths: ${Array.from(uniqueExpandWidths).join(', ')})`
  );

  // 3. Peek -> Dock handoff: sidebar must not retract or wobble, page width must transition smoothly
  await sidebar.evaluate("window.ipc.postMessage(JSON.stringify({action:'toggle_sidebar'}))");
  await new Promise(resolve => setTimeout(resolve, 500));

  // Hover to peek
  await sidebar.evaluate("document.documentElement.dispatchEvent(new MouseEvent('mouseenter'))");
  await new Promise(resolve => setTimeout(resolve, 400));
  assert.ok(await sidebar.evaluate('currentState.sidebar_peek'), 'Sidebar must peek on hover');

  await content.evaluate(`(() => {
    window.widths = [];
    window.sampler = setInterval(() => window.widths.push(innerWidth), 10);
    return true;
  })()`);

  // Dock while peeking
  await sidebar.evaluate("document.getElementById('btn-toggle-sidebar').click()");
  await new Promise(resolve => setTimeout(resolve, 500));

  const handoffSample = await content.evaluate(`(() => {
    clearInterval(window.sampler);
    return {
      widths: window.widths,
      finalWidth: innerWidth
    };
  })()`);

  const uniqueHandoffWidths = new Set(handoffSample.widths);
  assert.equal(
    handoffSample.finalWidth,
    initialWidth,
    `Page should return to docked width after peek-to-dock handoff (got ${handoffSample.finalWidth}, expected ${initialWidth})`
  );
  assert.ok(
    uniqueHandoffWidths.size >= 3,
    `Peek-to-dock handoff must produce intermediate frame widths (got ${uniqueHandoffWidths.size} unique widths)`
  );

  // 4. Side panel open/close transition
  const a = await sidebar.evaluate('currentState.active_tab_id');
  await sidebar.evaluate(
    `window.ipc.postMessage(JSON.stringify({action:'open_in_side_panel',id:${a}}))`
  );
  await new Promise(resolve => setTimeout(resolve, 500));

  await content.evaluate(`(() => {
    window.widths = [];
    window.sampler = setInterval(() => window.widths.push(innerWidth), 10);
    return true;
  })()`);

  // Toggle side panel to hide it
  await sidebar.evaluate("window.ipc.postMessage(JSON.stringify({action:'toggle_side_panel'}))");
  await new Promise(resolve => setTimeout(resolve, 500));

  const panelSample = await content.evaluate(`(() => {
    clearInterval(window.sampler);
    return {
      widths: window.widths,
      finalWidth: innerWidth
    };
  })()`);

  const uniquePanelWidths = new Set(panelSample.widths);
  assert.ok(
    uniquePanelWidths.size >= 3,
    `Side panel toggle transition must produce intermediate frame widths (got ${uniquePanelWidths.size} unique widths)`
  );

  const report = {
    initialWidth,
    collapsedWidth,
    collapseSampleCount: collapseSample.widths.length,
    collapseUniqueCount: uniqueCollapseWidths.size,
    expandSampleCount: expandSample.widths.length,
    expandUniqueCount: uniqueExpandWidths.size,
    handoffSampleCount: handoffSample.widths.length,
    handoffUniqueCount: uniqueHandoffWidths.size,
    panelSampleCount: panelSample.widths.length,
    panelUniqueCount: uniquePanelWidths.size,
  };

  if (artifacts) {
    await writeFile(join(artifacts, 'layout-transition.json'), JSON.stringify(report, null, 2));
  }

  console.log(
    `Smooth layout transitions verified: ${collapseSample.widths.length} collapse samples (${uniqueCollapseWidths.size} distinct intermediate steps) and ${expandSample.widths.length} expand samples (${uniqueExpandWidths.size} distinct intermediate steps).`
  );
} finally {
  closeConnections();
}
