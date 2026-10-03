// 最小 CDP 客户端:WebView2 只接受 Host 头为 [::1]:port 的直连,手写 WebSocket。
// 用法: node cdp-shot.mjs <port> <out.png> [js] [urlFilter]
// 可选第三参为 Runtime.evaluate 的表达式(先求值再截图,等待 800ms)。
import http from 'node:http';
import net from 'node:net';
import crypto from 'node:crypto';
import fs from 'node:fs';

const [port, out, js, urlFilter] = [process.argv[2], process.argv[3], process.argv[4], process.argv[5]];

function listOnce(host, port, headerHost) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host, port, path: '/json/list', method: 'GET', headers: { Host: headerHost } }, (res) => {
      let body = '';
      res.on('data', (chunk) => (body += chunk));
      res.on('end', () => { try { const parsed = JSON.parse(body); Array.isArray(parsed) && parsed.length ? resolve(parsed) : reject(new Error('no targets')); } catch (error) { reject(error); } });
    });
    req.on('error', reject);
    req.end();
  });
}

// WebView2 的调试端口有时绑 IPv6 回环、有时绑 127.0.0.1,两种都试。
function listTargets(port) {
  return listOnce('::1', port, `[::1]:${port}`).catch(() => listOnce('127.0.0.1', port, `127.0.0.1:${port}`));
}

class Cdp {
  constructor(socket) {
    this.socket = socket;
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.pending = new Map();
    socket.on('data', (chunk) => this.onData(chunk));
  }

  static connect(url) {
    const key = crypto.randomBytes(16).toString('base64');
    const { hostname, port, pathname } = new URL(url);
    return new Promise((resolve, reject) => {
      const socket = net.connect(Number(port), hostname === '[::1]' ? '::1' : hostname, () => {
        socket.write(`GET ${pathname} HTTP/1.1\r\nHost: ${hostname}:${port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${key}\r\nSec-WebSocket-Version: 13\r\n\r\n`);
      });
      let handshake = Buffer.alloc(0);
      const onHandshake = (chunk) => {
        handshake = Buffer.concat([handshake, chunk]);
        const marker = handshake.indexOf('\r\n\r\n');
        if (marker === -1) return;
        socket.removeListener('data', onHandshake);
        resolve(new Cdp(socket));
      };
      socket.on('data', onHandshake);
      socket.on('error', reject);
    });
  }

  onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 2) {
      const first = this.buffer[0];
      const len0 = this.buffer[1] & 0x7f;
      let offset = 2;
      let length = len0;
      if (len0 === 126) {
        if (this.buffer.length < 4) return;
        length = this.buffer.readUInt16BE(2);
        offset = 4;
      } else if (len0 === 127) {
        if (this.buffer.length < 10) return;
        length = Number(this.buffer.readBigUInt64BE(2));
        offset = 10;
      }
      if (this.buffer.length < offset + length) return;
      const payload = this.buffer.subarray(offset, offset + length);
      this.buffer = this.buffer.subarray(offset + length);
      const opcode = first & 0x0f;
      if (opcode === 9) this.sendFrame(10, payload); // ping→pong
      if (opcode === 1) {
        const message = JSON.parse(payload.toString('utf8'));
        if (message.id && this.pending.has(message.id)) {
          this.pending.get(message.id)(message);
          this.pending.delete(message.id);
        }
      }
    }
  }

  sendFrame(opcode, payload) {
    const mask = crypto.randomBytes(4);
    const masked = Buffer.from(payload);
    for (let i = 0; i < masked.length; i++) masked[i] ^= mask[i & 3];
    let header;
    if (payload.length < 126) {
      header = Buffer.from([0x80 | opcode, 0x80 | payload.length]);
    } else {
      header = Buffer.alloc(4);
      header[0] = 0x80 | opcode;
      header[1] = 0x80 | 126;
      header.writeUInt16BE(payload.length, 2);
    }
    this.socket.write(Buffer.concat([header, mask, masked]));
  }

  call(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`timeout ${method}`)); }, 15000);
      this.pending.set(id, (message) => {
        clearTimeout(timer);
        message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result);
      });
      this.sendFrame(1, Buffer.from(JSON.stringify({ id, method, params })));
    });
  }
}

const targets = await listTargets(port);
const page = urlFilter
  ? targets.find((target) => target.type === 'page' && target.url.includes(urlFilter))
  : targets.find((target) => target.type === 'page' && target.url.includes('index.html')) || targets.find((target) => target.type === 'page');
if (!page) { console.error('no page target:', targets.map((target) => target.url)); process.exit(1); }
const cdp = await Cdp.connect(page.webSocketDebuggerUrl);
await cdp.call('Page.enable');
if (js) {
  const evaluated = await cdp.call('Runtime.evaluate', { expression: js, awaitPromise: false, returnByValue: true });
  if (process.env.CDP_PRINT) console.log('eval:', JSON.stringify(evaluated.result?.value ?? evaluated.result));
  await new Promise((resolve) => setTimeout(resolve, 800));
}
const { data } = await cdp.call('Page.captureScreenshot', { format: 'png' });
fs.writeFileSync(out, Buffer.from(data, 'base64'));
console.log('saved', out);
process.exit(0);
