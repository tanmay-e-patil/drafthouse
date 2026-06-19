import http from 'k6/http';
import { check, sleep } from 'k6';
import exec from 'k6/execution';

const API_URL = __ENV.API_URL || 'https://drafthouse-api.tanmayep.dev';
const EMAIL = __ENV.LOADTEST_EMAIL;
const PASSWORD = __ENV.LOADTEST_PASSWORD;

export const options = {
  vus: Number(__ENV.VUS || 10),
  duration: __ENV.DURATION || '5m',
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<1000'],
  },
};

const jsonHeaders = { 'Content-Type': 'application/json' };
const docsByVu = {};
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

export default function () {
  if (!accessToken) login();
  const authHeaders = {
    ...jsonHeaders,
    Authorization: `Bearer ${accessToken}`,
  };
  const vu = exec.vu.idInTest;

  let docId = docsByVu[vu];
  if (!docId) {
    const create = http.post(
      `${API_URL}/documents`,
      JSON.stringify({ title: `loadtest-vu-${vu}-${Date.now()}` }),
      { headers: authHeaders, tags: { name: 'POST /documents' } }
    );
    check(create, { 'create document': (r) => r.status === 201 });
    docId = mustJson(create).id;
    docsByVu[vu] = docId;
  }

  const list = http.get(`${API_URL}/documents?limit=20`, {
    headers: authHeaders,
    tags: { name: 'GET /documents' },
  });
  check(list, { 'list documents': (r) => r.status === 200 });

  const get = http.get(`${API_URL}/documents/${docId}`, {
    headers: authHeaders,
    tags: { name: 'GET /documents/{id}' },
  });
  check(get, { 'get document': (r) => r.status === 200 });

  const patch = http.patch(
    `${API_URL}/documents/${docId}`,
    JSON.stringify({ title: `loadtest-vu-${vu}` }),
    { headers: authHeaders, tags: { name: 'PATCH /documents/{id}' } }
  );
  check(patch, { 'patch document': (r) => r.status === 200 });

  const content = http.get(`${API_URL}/documents/${docId}/content`, {
    headers: authHeaders,
    tags: { name: 'GET /documents/{id}/content' },
  });
  check(content, { 'get content': (r) => r.status === 200 });

  const updateContent = http.patch(
    `${API_URL}/documents/${docId}/content`,
    JSON.stringify({ content: `# Load test\n\nVU ${vu} at ${new Date().toISOString()}\n` }),
    { headers: authHeaders, tags: { name: 'PATCH /documents/{id}/content' } }
  );
  check(updateContent, { 'patch content': (r) => r.status === 200 });

  const ticket = http.post(`${API_URL}/documents/${docId}/ws-ticket`, null, {
    headers: authHeaders,
    tags: { name: 'POST /documents/{id}/ws-ticket' },
  });
  check(ticket, { 'issue ws ticket': (r) => r.status === 201 });

  sleep(Number(__ENV.SLEEP_SECONDS || 1));
}
