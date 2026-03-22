const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// ---------------------------------------------------------------------------
// DOM 参照
// ---------------------------------------------------------------------------
const aboutBtn        = document.getElementById('about-btn');
const toggleBtn       = document.getElementById('toggle-btn');
const toggleLabel     = document.getElementById('toggle-label');
const oscToggleBtn    = document.getElementById('osc-toggle-btn');
const oscToggleLabel  = document.getElementById('osc-toggle-label');
const settingsBtn     = document.getElementById('settings-btn');
const clearBtn        = document.getElementById('clear-btn');
const logList         = document.getElementById('log-list');
const errorBar        = document.getElementById('error-bar');
const errorBarMsg     = document.getElementById('error-bar-msg');
const errorBarClose   = document.getElementById('error-bar-close');
const oscStatusBar    = document.getElementById('osc-status');
const oscStatusText   = document.getElementById('osc-status-text');
const oscCancelBtn    = document.getElementById('osc-cancel-btn');
const fontDecreaseBtn = document.getElementById('font-decrease-btn');
const fontIncreaseBtn = document.getElementById('font-increase-btn');
const fontSizeValue   = document.getElementById('font-size-value');

// ---------------------------------------------------------------------------
// VirtualList — 可変高アイテムの仮想スクロール
//
// DOM 構造（logList の子）:
//   [emptyMsg]        ← 空状態メッセージ（件数0のときのみ表示）
//   [topPad]          ← 上スペーサー（レンダリング外アイテムの高さ分）
//   [rendered items]  ← 表示範囲内のアイテムのみ
//   [botPad]          ← 下スペーサー
// ---------------------------------------------------------------------------
class VirtualList {
  #container; #build;
  #items = []; #heights = [];
  #nodes = new Map();   // index → DOM node
  #topPad; #botPad;
  #EST = 110;  // 未測定アイテムの推定高さ (px)
  #GAP = 8;    // アイテム間ギャップ (margin-bottom と同値)
  #BUF = 5;    // ビューポート上下に余分にレンダリングするアイテム数
  #pending = false;

  constructor(container, buildFn) {
    this.#container = container;
    this.#build = buildFn;

    this.#topPad = document.createElement('div');
    this.#botPad = document.createElement('div');
    container.appendChild(this.#topPad);
    container.appendChild(this.#botPad);

    container.addEventListener('scroll', () => this.#schedule(), { passive: true });
    new ResizeObserver(() => this.#schedule()).observe(container);
  }

  /** 先頭に1件追加（新着翻訳） */
  prepend(item) {
    this.#items.unshift(item);
    this.#heights.unshift(0);
    // 既存ノードのインデックスを +1 シフト
    const m = new Map();
    for (const [i, n] of this.#nodes) m.set(i + 1, n);
    this.#nodes = m;
    // スクロール位置がトップ以外の場合、新アイテム分だけ補正してビューを安定させる
    if (this.#container.scrollTop > 0) {
      this.#container.scrollTop += this.#EST + this.#GAP;
    }
    this.#render();
  }

  /** 全件一括セット（初期ロード） */
  setAll(items) {
    for (const [, n] of this.#nodes) n.remove();
    this.#nodes.clear();
    this.#items = [...items];
    this.#heights = new Array(items.length).fill(0);
    this.#container.scrollTop = 0;
    this.#render();
  }

  /** 全件クリア */
  clear() {
    for (const [, n] of this.#nodes) n.remove();
    this.#nodes.clear();
    this.#items = [];
    this.#heights = [];
    this.#topPad.style.height = '0';
    this.#botPad.style.height = '0';
  }

  get count() { return this.#items.length; }

  // アイテム i が占める垂直スペース（高さ + ギャップ）
  #h(i) { return (this.#heights[i] || this.#EST) + this.#GAP; }

  #schedule() {
    if (this.#pending) return;
    this.#pending = true;
    requestAnimationFrame(() => { this.#pending = false; this.#render(); });
  }

  #render() {
    const n = this.#items.length;
    if (n === 0) {
      this.#topPad.style.height = '0';
      this.#botPad.style.height = '0';
      return;
    }

    const scrollTop = this.#container.scrollTop;
    const vh        = this.#container.clientHeight;

    // 累積高さの配列を一度だけ計算 (O(n)) — スペーサー計算に再利用
    const cum = new Array(n + 1);
    cum[0] = 0;
    for (let i = 0; i < n; i++) cum[i + 1] = cum[i] + this.#h(i);

    // ビューポートと交差するインデックス範囲を特定
    let start = n, end = -1;
    for (let i = 0; i < n; i++) {
      if (cum[i + 1] > scrollTop && start === n) start = i;
      if (cum[i] < scrollTop + vh) end = i;
    }
    if (end === -1) { start = 0; end = Math.min(n - 1, this.#BUF * 2); }
    start = Math.max(0, start - this.#BUF);
    end   = Math.min(n - 1, end + this.#BUF);

    // 範囲外のノードを DOM から除去
    for (const [i, node] of [...this.#nodes]) {
      if (i < start || i > end) { node.remove(); this.#nodes.delete(i); }
    }

    // 範囲内で未レンダリングのノードを生成・挿入
    for (let i = start; i <= end; i++) {
      if (this.#nodes.has(i)) continue;
      const node = this.#build(this.#items[i]);
      this.#nodes.set(i, node);

      // 挿入位置: i より大きいインデックスの中で最小のノードの直前
      let insertBefore = this.#botPad;
      let minJ = Infinity;
      for (const [j, jNode] of this.#nodes) {
        if (j > i && j < minJ) { minJ = j; insertBefore = jNode; }
      }
      this.#container.insertBefore(node, insertBefore);

      // レイアウト後に高さを記録
      if (node.offsetHeight > 0) this.#heights[i] = node.offsetHeight;
    }

    // レンダリング済みノードの実高さでキャッシュを更新
    for (const [i, node] of this.#nodes) {
      const h = node.offsetHeight;
      if (h > 0) this.#heights[i] = h;
    }

    // スペーサーの高さを更新（cum 配列を O(1) で参照）
    this.#topPad.style.height = cum[start] + 'px';
    this.#botPad.style.height = Math.max(0, cum[n] - cum[end + 1] - this.#GAP) + 'px';
  }
}

// ---------------------------------------------------------------------------
// 空状態メッセージ
// ---------------------------------------------------------------------------
const emptyMsg = (() => {
  const p = document.createElement('p');
  p.className = 'empty-state';
  p.textContent = '翻訳ログはまだありません';
  logList.prepend(p);  // topPad より前に置く
  return p;
})();

const showEmpty = () => { emptyMsg.hidden = false; };
const hideEmpty = () => { emptyMsg.hidden = true;  };

const vlist = new VirtualList(logList, buildLogEntry);

// ---------------------------------------------------------------------------
// 初期状態の読み込み
// ---------------------------------------------------------------------------
const FONT_MIN = 10;
const FONT_MAX = 20;
let currentFontSize = 13;

function applyFontSize(size) {
  currentFontSize = Math.min(FONT_MAX, Math.max(FONT_MIN, size));
  document.documentElement.style.setProperty('--font-size-base', currentFontSize + 'px');
  fontSizeValue.textContent = currentFontSize;
  fontDecreaseBtn.disabled = currentFontSize <= FONT_MIN;
  fontIncreaseBtn.disabled = currentFontSize >= FONT_MAX;
}

fontDecreaseBtn.addEventListener('click', () => {
  const next = currentFontSize - 1;
  if (next < FONT_MIN) return;
  applyFontSize(next);
  invoke('set_font_size', { size: next }).catch(() => {});
});

fontIncreaseBtn.addEventListener('click', () => {
  const next = currentFontSize + 1;
  if (next > FONT_MAX) return;
  applyFontSize(next);
  invoke('set_font_size', { size: next }).catch(() => {});
});

async function init() {
  try {
    const config = await invoke('get_config');
    isEnabled = config.is_enabled ?? false;
    updateToggleUI();
    setOscToggle(config.osc_enabled ?? true);
    soundEnabled = config.sound_enabled ?? true;
    applyFontSize(config.font_size ?? 13);
  } catch (e) {
    showError(`設定の読み込みに失敗しました: ${e}`);
  }

  try {
    const entries = await invoke('get_history');
    if (entries.length === 0) {
      showEmpty();
    } else {
      hideEmpty();
      vlist.setAll(entries);
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
// About ウィンドウを開く
// ---------------------------------------------------------------------------
aboutBtn.addEventListener('click', async () => {
  try {
    await invoke('open_about');
  } catch (e) {
    showError('About 画面を開けませんでした: ' + e);
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
    vlist.clear();
    showEmpty();
  } catch (e) {
    showError(`クリア失敗: ${e}`);
  }
});

// ---------------------------------------------------------------------------
// サムネイルクリックで画像を開く（イベント委譲）
// ---------------------------------------------------------------------------
logList.addEventListener('click', (e) => {
  const thumb = e.target.closest('.log-entry-thumb');
  if (!thumb) return;
  const path = thumb.dataset.path;
  if (path) invoke('open_file', { path });
});

// ---------------------------------------------------------------------------
// 翻訳中プレースホルダー
// ---------------------------------------------------------------------------
let pendingCount = 0;
let placeholderEl = null;

function buildPlaceholderEntry(imagePath) {
  const filename = imagePath.split(/[\\/]/).pop();
  const el = document.createElement('div');
  el.className = 'log-entry log-entry-pending';
  el.innerHTML = `
    <div class="log-entry-meta">
      <span class="log-entry-file" title="${escHtml(imagePath)}">${escHtml(filename)}</span>
    </div>
    <div class="pending-body">
      <span class="spinner"></span>
      <span class="pending-label">翻訳中...</span>
      <button class="pending-cancel-btn">キャンセル</button>
    </div>
  `;
  el.querySelector('.pending-cancel-btn').addEventListener('click', async (e) => {
    e.currentTarget.disabled = true;
    try { await invoke('cancel_translation'); } catch (_) {}
  });
  return el;
}

function showPlaceholder(imagePath) {
  pendingCount++;
  if (!placeholderEl) {
    placeholderEl = buildPlaceholderEntry(imagePath);
    logList.insertBefore(placeholderEl, emptyMsg.nextSibling);
  }
  const label = placeholderEl.querySelector('.pending-label');
  label.textContent = pendingCount > 1 ? `翻訳中... (${pendingCount}件)` : '翻訳中...';
  hideEmpty();
}

function hidePlaceholder() {
  pendingCount = Math.max(0, pendingCount - 1);
  if (pendingCount === 0 && placeholderEl) {
    placeholderEl.remove();
    placeholderEl = null;
  } else if (pendingCount > 0 && placeholderEl) {
    const label = placeholderEl.querySelector('.pending-label');
    label.textContent = `翻訳中... (${pendingCount}件)`;
  }
}

function resetPlaceholder() {
  pendingCount = 0;
  if (placeholderEl) {
    placeholderEl.remove();
    placeholderEl = null;
  }
}

// ---------------------------------------------------------------------------
// リアルタイムイベント
// ---------------------------------------------------------------------------
listen('translation_start', (event) => {
  showPlaceholder(event.payload);
});

let oscStatusTimer = null;

listen('osc_chunk_progress', (event) => {
  const { current, total } = event.payload;
  oscStatusText.textContent = `OSC送信中 (${current}/${total})`;
  oscCancelBtn.disabled = false;
  oscStatusBar.classList.remove('hidden');
  if (oscStatusTimer) clearTimeout(oscStatusTimer);
  if (current >= total) {
    oscCancelBtn.disabled = true;
    oscStatusTimer = setTimeout(() => {
      oscStatusBar.classList.add('hidden');
      oscStatusTimer = null;
    }, 2000);
  }
});

listen('osc_cancelled', () => {
  if (oscStatusTimer) clearTimeout(oscStatusTimer);
  oscStatusTimer = null;
  oscStatusBar.classList.add('hidden');
});

oscCancelBtn.addEventListener('click', async () => {
  oscCancelBtn.disabled = true;
  try { await invoke('cancel_osc'); } catch (_) {}
});

listen('translation_done', (event) => {
  hidePlaceholder();
  hideEmpty();
  vlist.prepend(event.payload);
  hideError();
  playNotification();
});

listen('translation_cancelled', () => {
  hidePlaceholder();
  if (vlist.count === 0) showEmpty();
});

listen('watcher_error', (event) => {
  resetPlaceholder();
  if (oscStatusTimer) clearTimeout(oscStatusTimer);
  oscStatusTimer = null;
  oscStatusBar.classList.add('hidden');
  if (vlist.count === 0) showEmpty();
  showError(event.payload);
});

listen('config_saved', async () => {
  hideError();
  try {
    const config = await invoke('get_config');
    isEnabled = config.is_enabled ?? true;
    updateToggleUI();
    setOscToggle(config.osc_enabled ?? true);
    soundEnabled = config.sound_enabled ?? true;
    applyFontSize(config.font_size ?? 13);
  } catch (_) {}
});

// ---------------------------------------------------------------------------
// ログ DOM 構築
// ---------------------------------------------------------------------------
function buildLogEntry(entry) {
  const filename = entry.image_path.split(/[\\/]/).pop();
  const time     = new Date(entry.timestamp).toLocaleTimeString('ja-JP');
  const model    = entry.model || '';
  const imgSrc   = convertFileSrc(entry.thumbnail_path ?? entry.image_path);

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
    <div class="log-entry-footer">
      <button class="btn-resend-osc" title="OSCで再送信">💬</button>
    </div>
  `;

  el.querySelector('.btn-resend-osc').addEventListener('click', async (e) => {
    const btn = e.currentTarget;
    btn.disabled = true;
    try {
      await invoke('resend_osc', { text: entry.translated_text });
    } catch (err) {
      showError(`OSC再送信に失敗しました: ${err}`);
    } finally {
      btn.disabled = false;
    }
  });

  return el;
}

// ---------------------------------------------------------------------------
// UI ヘルパー
// ---------------------------------------------------------------------------
function showError(msg) {
  errorBarMsg.textContent = msg;
  errorBar.classList.remove('hidden');
}

errorBarClose.addEventListener('click', hideError);

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
