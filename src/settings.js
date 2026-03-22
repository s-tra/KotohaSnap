const { invoke } = window.__TAURI__.core;

// ---------------------------------------------------------------------------
// DOM 参照
// ---------------------------------------------------------------------------
const providerSel       = document.getElementById('provider');
const customSection     = document.getElementById('custom-section');
const apiKeysSection    = document.getElementById('api-keys-section');
const customDisplayName = document.getElementById('custom-display-name');
const customApiUrl      = document.getElementById('custom-api-url');
const customApiKey      = document.getElementById('custom-api-key');
const customModelsUrl   = document.getElementById('custom-models-url');
const keyAnthropic      = document.getElementById('key-anthropic');
const keyOpenai         = document.getElementById('key-openai');
const keyGroq           = document.getElementById('key-groq');
const keyGoogle         = document.getElementById('key-google');
const keyAnthropicRow   = document.getElementById('key-anthropic-row');
const keyOpenaiRow      = document.getElementById('key-openai-row');
const keyGroqRow        = document.getElementById('key-groq-row');
const keyGoogleRow      = document.getElementById('key-google-row');
const modelInput        = document.getElementById('model-input');
const modelClearBtn     = document.getElementById('model-clear-btn');
const modelList         = document.getElementById('model-list');
const modelHint         = document.getElementById('model-hint');
const fetchModelsBtn    = document.getElementById('fetch-models-btn');
const watchDir          = document.getElementById('watch-dir');
const browseDirBtn      = document.getElementById('browse-dir-btn');
const oscHost           = document.getElementById('osc-host');
const oscPort           = document.getElementById('osc-port');
const oscAddress        = document.getElementById('osc-address');
const testOscBtn        = document.getElementById('test-osc-btn');
const oscChunkInterval  = document.getElementById('osc-chunk-interval');
const oscPrefixChk      = document.getElementById('osc-prefix-enabled');
const soundEnabledChk   = document.getElementById('sound-enabled');
const prompt            = document.getElementById('prompt');
const saveBtn           = document.getElementById('save-btn');
const saveStatus        = document.getElementById('save-status');
const resetConfigBtn    = document.getElementById('reset-config-btn');

// 設定保存時に osc_enabled を保持するための変数
let currentOscEnabled = true;

// プロバイダごとのモデル値（切り替え時に保持）
const providerModels = { anthropic: '', openai: '', groq: '', google: '', custom: '' };
let currentProvider = 'anthropic';

// プロバイダごとのデフォルトモデル（ヒント表示用）。バックエンドから起動時に取得する
let DEFAULT_MODELS = {};

// ---------------------------------------------------------------------------
// モデル入力欄のクリアボタン
// ---------------------------------------------------------------------------
function updateModelClearBtn() {
  modelClearBtn.hidden = modelInput.value === '';
}

modelInput.addEventListener('input', updateModelClearBtn);

modelClearBtn.addEventListener('click', () => {
  modelInput.value = '';
  updateModelClearBtn();
  modelInput.focus();
});

// ---------------------------------------------------------------------------
// プロバイダ切り替え
// ---------------------------------------------------------------------------
function onProviderChange() {
  // 切り替え前のモデル値を保存
  providerModels[currentProvider] = modelInput.value;
  currentProvider = providerSel.value;

  const isCustom = currentProvider === 'custom';
  customSection.style.display  = isCustom ? '' : 'none';
  apiKeysSection.style.display = isCustom ? 'none' : '';

  // 該当プロバイダのAPIキー行のみ表示
  keyAnthropicRow.style.display = currentProvider === 'anthropic' ? '' : 'none';
  keyOpenaiRow.style.display    = currentProvider === 'openai'    ? '' : 'none';
  keyGroqRow.style.display      = currentProvider === 'groq'      ? '' : 'none';
  keyGoogleRow.style.display    = currentProvider === 'google'    ? '' : 'none';

  // 該当プロバイダのモデル値をロード
  modelInput.value = providerModels[currentProvider];
  updateModelHint();
  updateModelClearBtn();
}

function updateModelHint() {
  const def = DEFAULT_MODELS[currentProvider] || '';
  modelHint.textContent = def ? `デフォルト: ${def}` : '任意のモデル名を入力してください';
  // datalist をリセット（プロバイダが変わったので）
  modelList.innerHTML = '';
}

providerSel.addEventListener('change', onProviderChange);


// ---------------------------------------------------------------------------
// 設定読み込み
// ---------------------------------------------------------------------------
async function loadConfig() {
  try {
    const [config, defaultModels] = await Promise.all([
      invoke('get_config'),
      invoke('get_default_models'),
    ]);
    DEFAULT_MODELS = defaultModels;

    // プロバイダごとのモデル値を先にロード
    providerModels.anthropic = config.models?.anthropic ?? '';
    providerModels.openai    = config.models?.openai    ?? '';
    providerModels.groq      = config.models?.groq      ?? '';
    providerModels.google    = config.models?.google    ?? '';
    providerModels.custom    = config.models?.custom    ?? '';

    keyAnthropic.value       = config.api_keys?.anthropic ?? '';
    keyOpenai.value          = config.api_keys?.openai    ?? '';
    keyGroq.value            = config.api_keys?.groq      ?? '';
    keyGoogle.value          = config.api_keys?.google    ?? '';
    customDisplayName.value  = config.custom_provider?.display_name ?? '';
    customApiUrl.value       = config.custom_provider?.api_url      ?? '';
    customApiKey.value       = config.custom_provider?.api_key      ?? '';
    customModelsUrl.value    = config.custom_provider?.models_url   ?? '';
    currentOscEnabled        = config.osc_enabled ?? true;
    oscPrefixChk.checked     = config.osc_prefix_enabled ?? false;
    soundEnabledChk.checked  = config.sound_enabled ?? true;
    watchDir.value           = config.watch_dir           ?? '';
    oscHost.value            = config.osc?.host                  ?? '127.0.0.1';
    oscPort.value            = config.osc?.port                  ?? 9000;
    oscAddress.value         = config.osc?.address               ?? '/chatbox/input';
    oscChunkInterval.value   = config.osc?.chunk_interval_secs   ?? 4;
    prompt.value             = config.translation_prompt  ?? '';

    // currentProvider をダミー値にしてから providerSel を設定し onProviderChange を呼ぶ
    // （ダミー値にすることで providerModels の読み込み済み値が上書きされない）
    currentProvider = '__init__';
    providerSel.value = config.provider ?? 'anthropic';
    onProviderChange();
  } catch (e) {
    showSaveStatus(`設定の読み込みに失敗しました: ${e}`, 'error');
  }
}

// ---------------------------------------------------------------------------
// フォームから Config を組み立て
// ---------------------------------------------------------------------------
function collectConfig() {
  // 現在表示中のモデル値を providerModels に反映してからコピー
  const models = { ...providerModels, [currentProvider]: modelInput.value.trim() };
  return {
    provider: providerSel.value,
    models,
    api_keys: {
      anthropic: keyAnthropic.value,
      openai:    keyOpenai.value,
      groq:      keyGroq.value,
      google:    keyGoogle.value,
    },
    custom_provider: {
      display_name: customDisplayName.value.trim(),
      api_url:      customApiUrl.value.trim(),
      api_key:      customApiKey.value,
      models_url:   customModelsUrl.value.trim(),
    },
    osc: {
      host:                oscHost.value,
      port:                parseInt(oscPort.value, 10) || 9000,
      address:             oscAddress.value,
      chunk_interval_secs: parseInt(oscChunkInterval.value, 10) || 4,
    },
    watch_dir:          watchDir.value,
    translation_prompt: prompt.value,
    // osc_enabled はメインウィンドウのトグルで管理するため現在値を保持
    osc_enabled:        currentOscEnabled,
    osc_prefix_enabled: oscPrefixChk.checked,
    sound_enabled:      soundEnabledChk.checked,
  };
}

// ---------------------------------------------------------------------------
// モデル一覧取得
// ---------------------------------------------------------------------------
fetchModelsBtn.addEventListener('click', async () => {
  fetchModelsBtn.disabled = true;
  fetchModelsBtn.textContent = '取得中...';
  modelList.innerHTML = '';

  const provider = providerSel.value;
  const apiKey = provider === 'custom'
    ? customApiKey.value
    : { anthropic: keyAnthropic.value, openai: keyOpenai.value, groq: keyGroq.value, google: keyGoogle.value }[provider] ?? '';
  const modelsUrl = provider === 'custom' ? customModelsUrl.value.trim() : null;

  try {
    const models = await invoke('fetch_models', { provider, apiKey, modelsUrl });
    const prev = modelInput.value;
    modelInput.value = '';
    models.forEach(id => {
      const opt = document.createElement('option');
      opt.value = id;
      modelList.appendChild(opt);
    });
    // 取得したモデルに現在値が含まれていれば復元、なければ空のまま
    if (prev && models.includes(prev)) modelInput.value = prev;
    updateModelClearBtn();
    modelInput.focus();
    showSaveStatus(`モデルを ${models.length} 件取得しました`, 'ok');
  } catch (e) {
    showSaveStatus(`モデル取得失敗: ${e}`, 'error');
  } finally {
    fetchModelsBtn.disabled = false;
    fetchModelsBtn.textContent = 'モデルを取得';
  }
});

// ---------------------------------------------------------------------------
// 保存
// ---------------------------------------------------------------------------
saveBtn.addEventListener('click', async () => {
  saveBtn.disabled = true;
  try {
    await invoke('save_config', { newConfig: collectConfig() });
    showSaveStatus('設定を保存しました', 'ok');
  } catch (e) {
    showSaveStatus(`保存失敗: ${e}`, 'error');
  } finally {
    saveBtn.disabled = false;
  }
});

// ---------------------------------------------------------------------------
// スクリーンショットフォルダ選択
// ---------------------------------------------------------------------------
browseDirBtn.addEventListener('click', async () => {
  const selected = await window.__TAURI__.dialog.open({
    directory: true,
    multiple: false,
    title: 'スクリーンショットフォルダを選択',
    defaultPath: watchDir.value || undefined,
  });
  if (selected) watchDir.value = selected;
});

// ---------------------------------------------------------------------------
// OSC テスト
// ---------------------------------------------------------------------------
testOscBtn.addEventListener('click', async () => {
  testOscBtn.disabled = true;
  try {
    await invoke('test_osc');
    showSaveStatus('OSC テスト送信しました', 'ok');
  } catch (e) {
    showSaveStatus(`OSC テスト失敗: ${e}`, 'error');
  } finally {
    testOscBtn.disabled = false;
  }
});

// ---------------------------------------------------------------------------
// UI ヘルパー
// ---------------------------------------------------------------------------
function showSaveStatus(msg, type) {
  saveStatus.textContent = msg;
  saveStatus.className = `save-status ${type}`;
  saveStatus.classList.remove('hidden');
  clearTimeout(showSaveStatus._timer);
  showSaveStatus._timer = setTimeout(() => saveStatus.classList.add('hidden'), 4000);
}

// ---------------------------------------------------------------------------
// 設定リセット
// ---------------------------------------------------------------------------
resetConfigBtn.addEventListener('click', async () => {
  const ok = await window.__TAURI__.dialog.confirm(
    'すべての設定内容が削除されます。本当によろしいですか？',
    { title: '設定のリセット', kind: 'warning' }
  );
  if (!ok) return;

  resetConfigBtn.disabled = true;
  try {
    await invoke('reset_config');
    await loadConfig();
    showSaveStatus('設定をリセットしました', 'ok');
  } catch (e) {
    showSaveStatus(`リセット失敗: ${e}`, 'error');
  } finally {
    resetConfigBtn.disabled = false;
  }
});

// ---------------------------------------------------------------------------
// 初期化
// ---------------------------------------------------------------------------
await loadConfig();
