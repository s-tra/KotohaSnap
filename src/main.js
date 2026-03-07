const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// ---------------------------------------------------------------------------
// DOM 参照
// ---------------------------------------------------------------------------
const toggleBtn      = document.getElementById('toggle-btn');
const toggleLabel    = document.getElementById('toggle-label');
const oscToggleBtn   = document.getElementById('osc-toggle-btn');
const oscToggleLabel = document.getElementById('osc-toggle-label');
const settingsBtn    = document.getElementById('settings-btn');
const clearBtn       = document.getElementById('clear-btn');
const logList        = document.getElementById('log-list');
const errorBar       = document.getElementById('error-bar');

// ---------------------------------------------------------------------------
// 初期状態の読み込み
// ---------------------------------------------------------------------------
async function init() {
  try {
    const config = await invoke('get_config');
    isEnabled = config.is_enabled ?? false;
    updateToggleUI();
    setOscToggle(config.osc_enabled ?? true);
    soundEnabled = config.sound_enabled ?? true;
  } catch (e) {
    showError(`設定の読み込みに失敗しました: ${e}`);
  }

  try {
    const entries = await invoke('get_history');
    logList.innerHTML = '';
    if (entries.length === 0) {
      appendEmptyState();
    } else {
      entries.forEach(appendLogEntry);
    }
  } catch (e) {
    showError(`履歴の読み込みに失敗しました: ${e}`);
  }
}

// ---------------------------------------------------------------------------
// 翻訳 ON/OFF トグル
// ---------------------------------------------------------------------------
let isEnabled = false;

toggleBtn.addEventListener('click', async () => {
  const next = !isEnabled;
  try {
    await invoke('set_enabled', { enabled: next });
    isEnabled = next;
    updateToggleUI();
  } catch (e) {
    showError(`トグル失敗: ${e}`);
  }
});

function updateToggleUI() {
  toggleBtn.setAttribute('aria-pressed', String(isEnabled));
  toggleLabel.textContent = isEnabled ? 'ON' : 'OFF';
  toggleLabel.classList.toggle('on', isEnabled);
}

// ---------------------------------------------------------------------------
// 通知音
// ---------------------------------------------------------------------------
let soundEnabled = true;

function playNotification() {
  if (!soundEnabled) return;
  try {
    const ctx = new AudioContext();
    const osc  = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.type = 'sine';
    osc.frequency.setValueAtTime(420, ctx.currentTime);
    gain.gain.setValueAtTime(0.10, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.15);
    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.15);
  } catch (_) {}
}

// ---------------------------------------------------------------------------
// OSC ON/OFF トグル
// ---------------------------------------------------------------------------
let oscEnabled = true;

function setOscToggle(val) {
  oscEnabled = val;
  oscToggleBtn.setAttribute('aria-pressed', String(oscEnabled));
  oscToggleLabel.textContent = oscEnabled ? 'ON' : 'OFF';
  oscToggleLabel.classList.toggle('on', oscEnabled);
}

oscToggleBtn.addEventListener('click', async () => {
  const next = !oscEnabled;
  try {
    await invoke('set_osc_enabled', { enabled: next });
    setOscToggle(next);
  } catch (e) {
    showError(`OSC トグル失敗: ${e}`);
  }
});

// ---------------------------------------------------------------------------
// 設定ウィンドウを開く
// ---------------------------------------------------------------------------
settingsBtn.addEventListener('click', async () => {
  try {
    await invoke('open_settings');
  } catch (e) {
    showError(`設定画面を開けませんでした: ${e}`);
  }
});

// ---------------------------------------------------------------------------
// 履歴クリア
// ---------------------------------------------------------------------------
clearBtn.addEventListener('click', async () => {
  try {
    await invoke('clear_history');
    logList.innerHTML = '';
    appendEmptyState();
  } catch (e) {
    showError(`クリア失敗: ${e}`);
  }
});

// ---------------------------------------------------------------------------
// サムネイルクリックで画像を開く
// ---------------------------------------------------------------------------
logList.addEventListener('click', (e) => {
  const thumb = e.target.closest('.log-entry-thumb');
  if (!thumb) return;
  const path = thumb.dataset.path;
  if (path) invoke('open_file', { path });
});

// ---------------------------------------------------------------------------
// リアルタイムイベント
// ---------------------------------------------------------------------------
listen('translation_done', (event) => {
  const entry = event.payload;
  const empty = logList.querySelector('.empty-state');
  if (empty) empty.remove();
  logList.insertBefore(buildLogEntry(entry), logList.firstChild);
  hideError();
  playNotification();
});

listen('watcher_error', (event) => {
  showError(event.payload);
});

// 設定保存時に OSC・サウンド状態を同期、エラーバーをリセット
listen('config_saved', async () => {
  hideError();
  try {
    const config = await invoke('get_config');
    setOscToggle(config.osc_enabled ?? true);
    soundEnabled = config.sound_enabled ?? true;
  } catch (_) {}
});

// ---------------------------------------------------------------------------
// ログ DOM 構築
// ---------------------------------------------------------------------------
function buildLogEntry(entry) {
  const filename = entry.image_path.split(/[\\/]/).pop();
  const time     = new Date(entry.timestamp).toLocaleTimeString('ja-JP');
  const model    = entry.model || '';
  const imgSrc   = convertFileSrc(entry.image_path);

  const el = document.createElement('div');
  el.className = 'log-entry';
  el.innerHTML = `
    <div class="log-entry-meta">
      <span class="log-entry-provider">${escHtml(entry.provider)}</span>
      ${model ? `<span class="log-entry-model">${escHtml(model)}</span>` : ''}
      <span class="log-entry-file" title="${escHtml(entry.image_path)}">${escHtml(filename)}</span>
      <span>${escHtml(time)}</span>
    </div>
    <div class="log-entry-body">
      <div class="log-entry-text">${escHtml(entry.translated_text)}</div>
      <img class="log-entry-thumb" src="${imgSrc}" alt="" loading="lazy" data-path="${escHtml(entry.image_path)}" title="クリックで画像を開く">
    </div>
  `;
  return el;
}

function appendLogEntry(entry) {
  logList.appendChild(buildLogEntry(entry));
}

function appendEmptyState() {
  const p = document.createElement('p');
  p.className = 'empty-state';
  p.textContent = '翻訳ログはまだありません';
  logList.appendChild(p);
}

// ---------------------------------------------------------------------------
// UI ヘルパー
// ---------------------------------------------------------------------------
function showError(msg) {
  errorBar.textContent = msg;
  errorBar.classList.remove('hidden');
}

function hideError() {
  errorBar.classList.add('hidden');
}

function escHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ---------------------------------------------------------------------------
// 初期化
// ---------------------------------------------------------------------------
await init();
