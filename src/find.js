// Find in page: a small overlay that drives Chromium's built-in window.find.
// Toggled on demand so no page pays for it unless the user asks.
(() => {
  const existing = document.getElementById('__veluno-find');
  if (existing) { existing.__velunoFocus?.(); return 'focused'; }
  if (!document.body) return 'unavailable';

  const host = document.createElement('div');
  host.id = '__veluno-find';
  host.style.cssText = 'all:initial;position:fixed;top:12px;right:12px;z-index:2147483647';
  const shadow = host.attachShadow({ mode: 'closed' });
  shadow.innerHTML = `<style>
:host{color-scheme:dark}
*{box-sizing:border-box}
.bar{display:flex;align-items:center;gap:6px;padding:8px;background:#171719;border:1px solid #3a3a40;border-radius:10px;box-shadow:0 12px 30px #0008;font:13px "Segoe UI",system-ui,sans-serif;color:#f5f5f7}
input{width:190px;height:28px;padding:0 8px;background:#0c0c0e;color:#f5f5f7;border:1px solid #3a3a40;border-radius:6px;font:inherit;outline:0}
input:focus{border-color:#dedee3}
.count{min-width:74px;color:#b2b2b8;font-size:12px}
button{height:28px;min-width:28px;padding:0 8px;background:#1b1b1e;color:#f5f5f7;border:1px solid #3a3a40;border-radius:6px;font:inherit;cursor:pointer}
button:hover{background:#27272b}
button:focus-visible{outline:2px solid #dedee3;outline-offset:2px}
</style><div class="bar" role="search" aria-label="Find in page"><input type="text" aria-label="Find in page" placeholder="Find in page" autocomplete="off" spellcheck="false"><span class="count" role="status" aria-live="polite"></span><button data-step="-1" aria-label="Previous match" title="Previous match (Shift+Enter)">↑</button><button data-step="1" aria-label="Next match" title="Next match (Enter)">↓</button><button data-close aria-label="Close find bar" title="Close (Esc)">✕</button></div>`;

  const input = shadow.querySelector('input');
  const count = shadow.querySelector('.count');
  const search = (text, backwards) => {
    if (!text) return null;
    try {
      return window.find(text, false, backwards, true, false, false, false);
    } catch {
      return null;
    }
  };
  const report = found => {
    count.textContent = found === null ? '' : found ? 'Match found' : 'No matches';
  };
  const step = backwards => {
    if (!input.value) { report(null); return; }
    report(search(input.value, backwards));
  };
  input.addEventListener('input', () => {
    report(input.value ? search(input.value, false) : null);
  });
  input.addEventListener('keydown', event => {
    event.stopPropagation();
    if (event.key === 'Escape') { event.preventDefault(); close(); }
    else if (event.key === 'Enter') { event.preventDefault(); step(event.shiftKey); }
  });
  for (const button of shadow.querySelectorAll('[data-step]')) {
    button.onclick = () => step(button.dataset.step === '-1');
  }
  shadow.querySelector('[data-close]').onclick = () => close();

  const focused = document.activeElement;
  const close = () => {
    host.remove();
    focused?.focus?.({ preventScroll: true });
  };
  host.__velunoFocus = () => {
    input.focus();
    input.select();
  };
  document.documentElement.append(host);
  host.__velunoFocus();
  return 'opened';
})()
