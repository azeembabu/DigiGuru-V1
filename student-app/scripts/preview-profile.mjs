/**
 * Dev-only visual check: drives headless Chrome over CDP to render
 * /student/profile against the running dev servers and screenshot it.
 *
 * Not part of the app bundle. Usage: node scripts/preview-profile.mjs
 */
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { setTimeout as sleep } from 'node:timers/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const APP = 'http://127.0.0.1:5199';
const API = 'http://localhost:4000/api';
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const PORT = 9333;
const OUT = 'scripts/__screens__';

const env = { ...process.env };
const chrome = spawn(
  CHROME,
  [
    '--headless=new',
    `--remote-debugging-port=${PORT}`,
    '--user-data-dir=' + join(tmpdir(), 'dg-chrome-profile'),
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-gpu',
    '--window-size=1440,1200',
    'about:blank',
  ],
  { env, stdio: 'ignore' },
);

async function cdpHttp(path) {
  const res = await fetch(`http://127.0.0.1:${PORT}${path}`);
  return res.json();
}

let ws;
let msgId = 0;
const pending = new Map();
let sessionId;

function send(method, params = {}, useSession = true) {
  const id = ++msgId;
  const payload = { id, method, params };
  if (useSession && sessionId) payload.sessionId = sessionId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify(payload));
  });
}

const consoleErrors = [];

async function main() {
  // Wait for CDP endpoint.
  let version;
  for (let i = 0; i < 50; i++) {
    try {
      version = await cdpHttp('/json/version');
      break;
    } catch {
      await sleep(200);
    }
  }
  if (!version) throw new Error('Chrome CDP endpoint never came up');

  ws = new WebSocket(version.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));

  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
      return;
    }
    if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') {
      consoleErrors.push(msg.params.args.map((a) => a.value ?? a.description).join(' '));
    }
    if (msg.method === 'Runtime.exceptionThrown') {
      consoleErrors.push(msg.params.exceptionDetails?.exception?.description ?? 'exception');
    }
  };

  const { targetId } = await send('Target.createTarget', { url: 'about:blank' }, false);
  const attached = await send('Target.attachToTarget', { targetId, flatten: true }, false);
  sessionId = attached.sessionId;

  await send('Page.enable');
  await send('Runtime.enable');

  // Authenticate via the API, then seed the app's session storage.
  const loginRes = await fetch(`${API}/auth/student/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ identifier: 'STUDENT001', password: 'Student@12345' }),
  });
  const login = await loginRes.json();
  if (!login.success) throw new Error('Login failed: ' + JSON.stringify(login));
  const { accessToken, refreshToken, user, student } = login.data;

  const appUser = {
    id: user.id,
    fullName: student.full_name,
    rollNumber: student.roll_number,
    email: user.email,
    phoneNumber: student.phone_number,
    program: student.program?.name ?? '',
    semester: student.semester?.name ?? '',
    lsc: student.lsc?.name ?? '',
    role: 'STUDENT',
  };

  async function navigate(url) {
    await send('Page.navigate', { url });
    await sleep(1400);
  }

  async function setViewport(width, height) {
    await send('Emulation.setDeviceMetricsOverride', {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: width < 640,
    });
  }

  async function screenshot(name) {
    const { data } = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
    await writeFile(`${OUT}/${name}.png`, Buffer.from(data, 'base64'));
    console.log('saved', `${OUT}/${name}.png`);
  }

  async function textOf(selector) {
    const { result } = await send('Runtime.evaluate', {
      expression: `document.querySelector(${JSON.stringify(selector)})?.innerText ?? null`,
      returnByValue: true,
    });
    return result.value;
  }

  await mkdir(OUT, { recursive: true });

  // Seed tokens on the app origin.
  await navigate(`${APP}/login`);
  await send('Runtime.evaluate', {
    expression: `(function(){
      localStorage.setItem('dg_access_token', ${JSON.stringify(accessToken)});
      localStorage.setItem('dg_refresh_token', ${JSON.stringify(refreshToken)});
      localStorage.setItem('dg_user', ${JSON.stringify(JSON.stringify(appUser))});
      return true;
    })()`,
    returnByValue: true,
  });

  // ── Desktop ──
  await setViewport(1440, 1200);
  await navigate(`${APP}/student/profile`);
  await sleep(1200);
  await screenshot('profile-desktop');

  const bodyText = await textOf('body');
  console.log('--- body text (truncated) ---');
  console.log((bodyText ?? '').slice(0, 900));

  // ── Mobile ──
  await setViewport(390, 900);
  await navigate(`${APP}/student/profile`);
  await sleep(1000);
  await screenshot('profile-mobile');

  console.log('--- console errors ---');
  console.log(consoleErrors.length ? consoleErrors.join('\n') : 'none');

  ws.close();
  chrome.kill();
}

main().catch((err) => {
  console.error('preview failed:', err);
  chrome.kill();
  process.exit(1);
});
