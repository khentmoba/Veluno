const [port, expression, targetUrl, text] = process.argv.slice(2);
let lastError;
const deadline = Date.now() + 10000;
while (Date.now() < deadline) {
  let targets;
  try {
    targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  } catch (error) {
    if (error.cause?.code !== 'ECONNREFUSED') throw error;
    lastError = error;
    await new Promise(resolve => setTimeout(resolve, 100));
    continue;
  }
  for (const target of targets.filter(target => (target.type === 'page' || targetUrl && target.type === 'iframe') && (!targetUrl || target.url === targetUrl))) {
    const socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
    let id = 0;
    const pending = new Map();
    socket.onmessage = event => {
      const message = JSON.parse(event.data);
      pending.get(message.id)?.(message);
      pending.delete(message.id);
    };
    const send = (method, params) => new Promise(resolve => {
      const request = ++id;
      pending.set(request, resolve);
      socket.send(JSON.stringify({ id: request, method, params }));
    });
    const evaluate = expression => send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
    const probe = await evaluate(targetUrl ? 'true' : "Boolean(document.getElementById('url-input') && window.ipc?.postMessage && window.updateState)");
    if (probe.result?.result?.value) {
      if (targetUrl) await send('Page.bringToFront');
      const response = await evaluate(expression);
      // Type like a user into whatever the expression focused.
      if (text) await send('Input.insertText', { text });
      socket.close();
      if (response.error || response.result?.exceptionDetails) throw Error(JSON.stringify(response));
      console.log(JSON.stringify(response.result.result.value ?? null));
      process.exit(0);
    }
    socket.close();
  }
  await new Promise(resolve => setTimeout(resolve, 100));
}
throw Error('Native sidebar debugger was not ready within 10 seconds', { cause: lastError });
