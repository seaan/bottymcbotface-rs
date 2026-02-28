const tokenEl = document.getElementById('month-token');
const loginPanel = document.getElementById('login-panel');
const calendarPanel = document.getElementById('calendar-panel');
const loginStatus = document.getElementById('login-status');
const saveStatus = document.getElementById('save-status');
const grid = document.getElementById('calendar-grid');

const monthToken = tokenEl.dataset.monthToken;
let editToken = localStorage.getItem(`dnd_edit_${monthToken}`) || null;
let selected = new Set();
let dragMode = null;
let isDragging = false;
let pending = { add: new Set(), remove: new Set() };
let selectedMonthName = null;

function monthDates(monthName) {
  const now = new Date();
  const monthIndex = {
    january: 0,
    february: 1,
    march: 2,
    april: 3,
    may: 4,
    june: 5,
    july: 6,
    august: 7,
    september: 8,
    october: 9,
    november: 10,
    december: 11,
  }[String(monthName || '').toLowerCase()] ?? now.getMonth();

  const base = new Date(now.getFullYear(), monthIndex, 1);
  const daysInMonth = new Date(base.getFullYear(), monthIndex + 1, 0).getDate();
  return [...Array(daysInMonth).keys()].map((idx) => {
    const d = new Date(base.getFullYear(), monthIndex, idx + 1);
    return d.toISOString().slice(0, 10);
  });
}

function markDay(date, active) {
  const el = document.querySelector(`[data-date="${date}"]`);
  if (!el) return;
  el.classList.toggle('selected', active);
}

function queueAutosave() {
  clearTimeout(window.__dndAutosave);
  window.__dndAutosave = setTimeout(flushAutosave, 200);
}

async function flushAutosave() {
  if (!editToken) return;
  if (!pending.add.size && !pending.remove.size) return;

  const payload = {
    edit_token: editToken,
    set_available: [...pending.add],
    set_unavailable: [...pending.remove],
  };

  pending = { add: new Set(), remove: new Set() };
  saveStatus.textContent = 'Saving...';

  const response = await fetch('/api/dnd/availability', {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    const message = await response.text();
    saveStatus.textContent = `Save failed: ${message}`;
    return;
  }

  saveStatus.textContent = 'Saved';
}

function toggleDate(date) {
  if (selected.has(date)) {
    selected.delete(date);
    pending.remove.add(date);
    pending.add.delete(date);
    markDay(date, false);
    if (dragMode === null) dragMode = 'remove';
  } else {
    selected.add(date);
    pending.add.add(date);
    pending.remove.delete(date);
    markDay(date, true);
    if (dragMode === null) dragMode = 'add';
  }
  queueAutosave();
}

function applyDrag(date) {
  if (!isDragging || dragMode === null) return;
  if (dragMode === 'add' && !selected.has(date)) {
    selected.add(date);
    pending.add.add(date);
    pending.remove.delete(date);
    markDay(date, true);
    queueAutosave();
  }
  if (dragMode === 'remove' && selected.has(date)) {
    selected.delete(date);
    pending.remove.add(date);
    pending.add.delete(date);
    markDay(date, false);
    queueAutosave();
  }
}

function renderCalendar() {
  grid.innerHTML = '';
  monthDates(selectedMonthName).forEach((date) => {
    const el = document.createElement('button');
    el.type = 'button';
    el.className = 'day';
    el.dataset.date = date;
    el.textContent = date;

    el.addEventListener('mousedown', (event) => {
      event.preventDefault();
      isDragging = true;
      dragMode = null;
      toggleDate(date);
    });

    el.addEventListener('mouseenter', () => applyDrag(date));
    grid.appendChild(el);
  });

  document.addEventListener('mouseup', () => {
    isDragging = false;
    dragMode = null;
  });
}

async function onLogin() {
  const name = document.getElementById('name').value.trim();
  const code = document.getElementById('code').value.trim();

  const response = await fetch('/api/dnd/login', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ month_token: monthToken, name, code }),
  });

  if (!response.ok) {
    loginStatus.textContent = 'Login failed. Check name/code.';
    return;
  }

  const payload = await response.json();
  editToken = payload.edit_token;
  localStorage.setItem(`dnd_edit_${monthToken}`, editToken);
  selected = new Set(payload.selected_dates);
  selectedMonthName = payload.month_name;

  loginPanel.hidden = true;
  calendarPanel.hidden = false;
  renderCalendar();
  selected.forEach((date) => markDay(date, true));

  if (payload.readonly) {
    saveStatus.textContent = 'This month is read-only.';
    grid.style.pointerEvents = 'none';
  }
}

document.getElementById('login-btn').addEventListener('click', onLogin);
