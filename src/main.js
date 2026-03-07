const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// ---------------------------------------------------------------------------
// DOM 参照
// ---------------------------------------------------------------------------
const aboutBtn       = document.getElementById('about-btn');
const toggleBtn      = document.getElementById('toggle-btn');
const toggleLabel    = document.getElementById('toggle-label');
const oscToggleBtn   = document.getElementById('osc-toggle-btn');
const oscToggleLabel = document.getElementById('osc-toggle-label');
const settingsBtn    = document.getElementById('settings-btn');
const clearBtn       = document.getElementById('clear-btn');
const logList        = document.getElementById('log-list');
const errorBar       = document.getElementById('error-bar');

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

  // インデックス 0 からアイテム i の直前までの累積高さ
  #cumTop(i) {
    let t = 0;
    for (let j = 0; j < i; j++) t += this.#h(j);
    return t;
  }

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

    // ビューポートと交差するインデックス範囲を特定
    let start = n, end = -1, acc = 0;
    for (let i = 0; i < n; i++) {
      const h = this.#h(i);
      if (acc + h > scrollTop && start === n) start = i;
      if (acc < scrollTop + vh) end = i;
      acc += h;
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

    // スペーサーの高さを更新
    const topH = this.#cumTop(start);
    const botH = Math.max(0, this.#cumTop(n) - this.#cumTop(end + 1) - this.#GAP);
    this.#topPad.style.height = topH + 'px';
    this.#botPad.style.height = botH + 'px';
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
// リアルタイムイベント
// ---------------------------------------------------------------------------
listen('translation_done', (event) => {
  hideEmpty();
  vlist.prepend(event.payload);
  hideError();
  playNotification();
});

listen('watcher_error', (event) => {
  showError(event.payload);
});

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
  `;
  return el;
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
