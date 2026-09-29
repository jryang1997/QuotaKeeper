// WebView2 exposes postMessage and invokes named callbacks from the Rust shell.
// Keep that wire format in one place so pages only pass command objects.
export function sendCommand(command, host = globalThis) {
  host.ipc?.postMessage?.(JSON.stringify(command));
}

export function registerNativeCallbacks(callbacks, host = globalThis) {
  for (const [name, handler] of Object.entries(callbacks)) host[name] = handler;
  return () => {
    for (const [name, handler] of Object.entries(callbacks)) {
      if (host[name] === handler) delete host[name];
    }
  };
}
