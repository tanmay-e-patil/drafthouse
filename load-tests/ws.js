import http from 'k6/http';
import ws from 'k6/ws';
import { check } from 'k6';

const API_URL = __ENV.API_URL || 'https://drafthouse-api.tanmayep.dev';
const WS_URL = API_URL.replace(/^http/, 'ws');
const EMAIL = __ENV.LOADTEST_EMAIL;
const PASSWORD = __ENV.LOADTEST_PASSWORD;

export const options = {
  vus: Number(__ENV.VUS || 10),
  duration: __ENV.DURATION || '5m',
  thresholds: {
    checks: ['rate>0.99'],
    ws_connecting: ['p(95)<1000'],
  },
};

const jsonHeaders = { 'Content-Type': 'application/json' };
let accessToken;

function mustJson(res) {
  try {
    return res.json();
  } catch (_) {
    return {};
  }
}

function login() {
  if (!EMAIL || !PASSWORD) {
    throw new Error('Set LOADTEST_EMAIL and LOADTEST_PASSWORD for a verified seeded user.');
  }

  const res = http.post(
    `${API_URL}/auth/login`,
    JSON.stringify({ email: EMAIL, password: PASSWORD }),
    { headers: jsonHeaders }
  );
  check(res, { 'login ok': (r) => r.status === 200 });
  accessToken = mustJson(res).access_token;
  if (!accessToken) throw new Error(`Login failed: ${res.status} ${res.body}`);
}

export function setup() {
  login();

  const create = http.post(
    `${API_URL}/documents`,
    JSON.stringify({ title: `loadtest-ws-${Date.now()}` }),
    { headers: { ...jsonHeaders, Authorization: `Bearer ${accessToken}` } }
  );
  check(create, { 'create hot doc': (r) => r.status === 201 });
  const docId = mustJson(create).id;
  if (!docId) throw new Error(`Document create failed: ${create.status} ${create.body}`);

  return { docId };
}

export default function ({ docId }) {
  if (!accessToken) login();
  const ticketRes = http.post(`${API_URL}/documents/${docId}/ws-ticket`, null, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  check(ticketRes, { 'ticket issued': (r) => r.status === 201 });
  const ticket = mustJson(ticketRes).ticket;
  if (!ticket) return;

  const url = `${WS_URL}/collab/${docId}?ticket=${encodeURIComponent(ticket)}`;
  const res = ws.connect(url, {}, (socket) => {
    socket.on('open', () => {
      socket.setInterval(() => socket.ping(), 5000);
      socket.setTimeout(() => socket.close(), Number(__ENV.WS_HOLD_MS || 30000));
    });
    socket.on('error', (e) => console.error(`ws error: ${e.error()}`));
  });

  check(res, { 'ws upgraded': (r) => r && r.status === 101 });
}
