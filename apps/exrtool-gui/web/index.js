(() => {
  // 定数
  const MAX_PREVIEW = 2048;          // プレビュー最大解像度
  const STATS_BINS = 256;            // ヒストグラム/波形のビン数
  const UPDATE_DEBOUNCE_MS = 120;    // プレビュー更新のデバウンス
  let invoke = null; // 解決済みの invoke（nullなら未解決）
  let imgW = 0, imgH = 0;
  let useStateLutEnabled = true; // LUT in-memory 使用フラグ（既定ON固定）
  let pipetteFixed = false; // スポイト固定
  let stats = null;
  let waveform = null;
  let scopeChannel = 'rgb';
  let scopeScale = 1;
  let previewMode = 'rgb';
  let compareMode = false;
  let currImgData = null;
  let prevImgData = null;

  // 簡易デバウンス
  function debounce(fn, ms) {
    let t = null;
    return (...args) => {
      if (t) clearTimeout(t);
      t = setTimeout(() => fn(...args), ms);
    };
  }

  function getEl(id) {
    const el = document.getElementById(id);
    if (!el) console.error(`element #${id} not found`);
    return el;
  }

  function appendLog(msg) {
    const logbox = document.getElementById('logbox');
    const ts = new Date().toISOString();
    const line = `[${ts}] ${msg}\n`;
    if (logbox) {
      logbox.textContent += line;
      logbox.scrollTop = logbox.scrollHeight;
    }
    console.log(line);
  }

  // UI + ファイル両方へログ（可能なら）
  async function logBoth(msg) {
    appendLog(msg);
    try { if (invoke || await ensureTauriReady()) { await invoke('write_log', { s: msg }); } } catch (_) {}
  }

  function showError(msg) {
    let el = document.getElementById('errordiv');
    if (!el) {
      el = document.createElement('div');
      el.id = 'errordiv';
      el.style.color = 'red';
      document.body.appendChild(el);
    }
    el.textContent = msg;
  }

  async function ensureTauriReady(timeoutMs = 5000) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      const t = window.__TAURI__;
      if (t) {
        const inv = t.invoke || (t.tauri && t.tauri.invoke) || (t.core && t.core.invoke);
        if (inv) { invoke = inv; return true; }
      }
      await new Promise(r => setTimeout(r, 50));
    }
    appendLog('Tauri API 解決に失敗');
    return false;
  }

  // ----- 左パネル: メディアリスト -----
  const leftPanel = getEl('left-panel');
  const fileList = getEl('file-list');
  const importBtn = getEl('import-media');
  let draggedItem = null;

  function basename(path) { return path.replace(/^.*[\\/]/, ''); }

  function addFileItem(path) {
    if (!fileList) return;
    const li = document.createElement('li');
    li.textContent = basename(path);
    li.dataset.path = path;
    li.classList.add('file');
    li.draggable = true;
    li.addEventListener('dragstart', () => { draggedItem = li; });
    fileList.appendChild(li);
  }

  async function importFiles(paths) {
    for (const p of paths) addFileItem(p);
  }

  if (importBtn) importBtn.addEventListener('click', async () => {
    try {
      if (!(await ensureTauriReady())) return;
      const t = window.__TAURI__;
      const openDlg = (t && t.dialog && t.dialog.open) || (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open);
      if (!openDlg) return;
      const res = await openDlg({ multiple: true });
      if (res) {
        const arr = Array.isArray(res) ? res : [res];
        await importFiles(arr);
      }
    } catch (e) { appendLog('import failed: ' + e); }
  });

  if (leftPanel) {
    leftPanel.addEventListener('dragover', e => e.preventDefault());
    leftPanel.addEventListener('drop', e => {
      e.preventDefault();
      if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
        const arr = [];
        for (const f of e.dataTransfer.files) arr.push(f.path || f.name);
        importFiles(arr);
      } else if (draggedItem) {
        const folder = e.target.closest('li.folder');
        if (folder) folder.querySelector('ul').appendChild(draggedItem);
        else if (fileList) fileList.appendChild(draggedItem);
        draggedItem = null;
      }
    });
  }

  if (fileList) fileList.addEventListener('click', async e => {
    const li = e.target.closest('li.file');
    if (!li) return;
    fileList.querySelectorAll('li.selected').forEach(el => el.classList.remove('selected'));
    li.classList.add('selected');
    const pathEl = getEl('path');
    if (pathEl) pathEl.value = li.dataset.path || '';
    await openExr();
  });

  // コンテキストメニュー
  let ctxMenu = null;
  function closeCtxMenu() { if (ctxMenu) { ctxMenu.remove(); ctxMenu = null; } }
  document.addEventListener('click', closeCtxMenu);
  if (leftPanel) leftPanel.addEventListener('contextmenu', e => {
    e.preventDefault();
    closeCtxMenu();
    ctxMenu = document.createElement('div');
    ctxMenu.id = 'context-menu';
    ctxMenu.style.left = `${e.pageX}px`;
    ctxMenu.style.top = `${e.pageY}px`;
    const targetLi = e.target.closest('li');
    if (targetLi && targetLi.classList.contains('file')) {
      const del = document.createElement('div');
      del.textContent = 'Delete';
      del.addEventListener('click', () => { targetLi.remove(); closeCtxMenu(); });
      ctxMenu.appendChild(del);
    } else {
      const newFolder = document.createElement('div');
      newFolder.textContent = 'New Folder';
      newFolder.addEventListener('click', () => {
        const name = prompt('フォルダ名'); if (!name) return;
        const li = document.createElement('li');
        li.classList.add('folder');
        li.innerHTML = `<span>${name}</span><ul></ul>`;
        li.addEventListener('dragover', ev => ev.preventDefault());
        li.addEventListener('drop', ev => { ev.preventDefault(); if (draggedItem) { li.querySelector('ul').appendChild(draggedItem); draggedItem = null; } });
        if (fileList) fileList.appendChild(li);
        closeCtxMenu();
      });
      ctxMenu.appendChild(newFolder);
    }
    document.body.appendChild(ctxMenu);
  });

  function drawHistogram(s) {
    const cv = getEl('hist');
    if (!cv || !s) return;
    const ctx = cv.getContext('2d');
    ctx.clearRect(0,0,cv.width,cv.height);
    const channels = scopeChannel === 'rgb' ? ['r','g','b'] : [scopeChannel];
    const colors = { r:'red', g:'green', b:'blue' };
    for (const ch of channels) {
      const hist = s['hist_' + ch];
      if (!hist) continue;
      const max = Math.max(...hist) || 1;
      ctx.strokeStyle = colors[ch];
      ctx.beginPath();
      hist.forEach((v,i)=>{
        const h = Math.min(cv.height, (v/max)*cv.height*scopeScale);
        ctx.moveTo(i, cv.height);
        ctx.lineTo(i, cv.height - h);
      });
      ctx.stroke();
    }
  }

  function drawWaveform(wf) {
    const cv = getEl('waveform');
    if (!cv || !wf) return;
    const ctx = cv.getContext('2d');
    const width = cv.width;
    const height = cv.height;
    ctx.clearRect(0,0,width,height);
    const channels = scopeChannel === 'rgb' ? ['r','g','b'] : [scopeChannel];
    const colors = { r:'red', g:'green', b:'blue' };
    const xb = wf.x_bins;
    const yb = wf.y_bins;
    const sx = width / xb;
    const sy = height / yb;
    ctx.globalAlpha = 1;
    for (const ch of channels) {
      const arr = wf[ch];
      if (!arr) continue;
      ctx.fillStyle = colors[ch];
      for (let x=0; x<xb; x++) {
        for (let y=0; y<yb; y++) {
          const c = arr[x*yb + y];
          if (c>0) {
            const alpha = Math.min(1, c * scopeScale / 10);
            ctx.globalAlpha = alpha;
            ctx.fillRect(x*sx, height - (y+1)*sy, sx, sy);
          }
        }
      }
    }
    ctx.globalAlpha = 1;
  }

  async function refreshScopes() {
    try {
      if (!(invoke || await ensureTauriReady())) return;
      const [s, wf] = await Promise.all([
        invoke('image_stats'),
        invoke('image_waveform'),
      ]);
      stats = s; waveform = wf;
      drawHistogram(stats);
      drawWaveform(waveform);
    } catch (e) { console.error('refreshScopes failed', e); }
  }

  function updateInfoText() {
    const info = getEl('info');
    if (!info) return;
    const mode = previewMode === 'rgb' ? 'RGB' : 'A';
    const ab = compareMode ? 'B' : 'A';
    info.textContent = `preview: ${imgW}x${imgH} (${mode}/${ab})`;
  }

  function renderPreview() {
    const cv = getEl('cv');
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const src = (compareMode && prevImgData) ? prevImgData : currImgData;
    if (!src) return;
    const data = new Uint8ClampedArray(src.data);
    if (previewMode === 'a') {
      for (let i = 0; i < data.length; i += 4) {
        const a = data[i + 3];
        data[i] = data[i + 1] = data[i + 2] = a;
        data[i + 3] = 255;
      }
    }
    const imgData = new ImageData(data, src.width, src.height);
    ctx.putImageData(imgData, 0, 0);
    updateInfoText();
  }

  async function openExr() {
    const pathEl = getEl('path');
    // OCIO要素の参照は必要時のみ取得（openExr内では未使用）
    const cv = getEl('cv');
    const info = getEl('info');
    if (!pathEl || !cv || !info) return;
    const ctx = cv.getContext('2d');

    const path = pathEl.value.trim();
    const lutPath = null; // 外部LUT読込は廃止
    try {
      if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
      const [w, h, b64] = await invoke('open_exr', {
        path,
        // 最大プレビュー解像度（既定）
        maxSize: MAX_PREVIEW,
        exposure: 0,
        gamma: 1.0,
        lutPath: lutPath,
        highQuality: true
      });
      const img = new Image();
      img.onload = () => {
        imgW = w; imgH = h;
        cv.width = w; cv.height = h;
        ctx.clearRect(0,0,w,h);
        ctx.drawImage(img, 0, 0);
        prevImgData = currImgData;
        currImgData = ctx.getImageData(0,0,w,h);
        compareMode = false;
        getEl('btn-compare')?.classList.remove('active');
        renderPreview();
        appendLog(`open ok: ${w}x${h}`);
      };
      img.src = 'data:image/png;base64,' + b64;
      await loadMetadata(path);
      await refreshScopes();
    } catch (e) {
      if (String(e).includes('cancelled')) {
        appendLog('読み込みキャンセル');
      } else {
        appendLog('読み込み失敗: ' + e);
        alert('読み込み失敗: ' + e);
      }
    } finally {
      // progress UI は未配線のため no-op
    }
  }

  async function loadMetadata(path) {
    if (!attrTable) return;
    const tbody = attrTable.querySelector('tbody');
    if (!tbody) return;
    try {
      if (!(await ensureTauriReady())) return;
      const res = await invoke('read_metadata', { path });
      const entries = Array.isArray(res) ? res : Object.entries(res);
      tbody.innerHTML = '';
      for (const [name, value] of entries) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td class="name"></td><td class="value"></td>`;
        tr.querySelector('.name').textContent = String(name);
        tr.querySelector('.value').textContent = String(value);
        tbody.appendChild(tr);
      }
    } catch (e) { appendLog('metadata読み込み失敗: ' + e); }
  }

  document.addEventListener('DOMContentLoaded', () => {
    // Tabs
    const tabBtnPreview = document.getElementById('tab-btn-preview');
    const tabBtnSettings = document.getElementById('tab-btn-settings');
    const tabPreview = document.getElementById('tab-preview');
    const tabSettings = document.getElementById('tab-settings');
    function activate(tab){
      if (!tabPreview || !tabSettings) return;
      tabPreview.style.display = (tab === 'preview') ? 'block' : 'none';
      tabSettings.style.display = (tab === 'settings') ? 'block' : 'none';
      tabBtnPreview?.classList.toggle('active', tab === 'preview');
      tabBtnSettings?.classList.toggle('active', tab === 'settings');
    }
    tabBtnPreview?.addEventListener('click', ()=>{ logBoth('tab: preview'); activate('preview'); });
    tabBtnSettings?.addEventListener('click', ()=>{ logBoth('tab: settings'); activate('settings'); });
    const openBtn = getEl('open');
    const browseBtn = getEl('browse');
    const saveBtn = getEl('save');
    const cv = getEl('cv');
    const pathEl = getEl('path');
    // HQ/LUT UIは廃止（既定ON）
    const hqEl = null;
    const lutSize = null;
    const applyTransformBtn = null;
    const clearLutBtn = null;
    const useStateLut = null;
    const progIntervalEl = getEl('progress-interval');
    const progIntervalResetBtn = getEl('progress-interval-reset');
    const progThreshEl = getEl('progress-threshold');
    const progThreshResetBtn = getEl('progress-threshold-reset');
    const defaultTransformEl = getEl('default-transform');
    const logConsentEl = getEl('log-consent');
    attrTable = getEl('attr-table');
    const scopeChannelEl = getEl('scope-channel');
    const scopeScaleEl = getEl('scope-scale');
    const viewRgbBtn = getEl('btn-view-rgb');
    const viewAlphaBtn = getEl('btn-view-alpha');
    const compareBtn = getEl('btn-compare');

    useStateLutEnabled = true;
    scopeChannelEl?.addEventListener('change', () => {
      scopeChannel = scopeChannelEl.value;
      drawHistogram(stats);
      drawWaveform(waveform);
    });
    scopeScaleEl?.addEventListener('change', () => {
      scopeScale = parseInt(scopeScaleEl.value)||1;
      drawHistogram(stats);
      drawWaveform(waveform);
    });

    function setChannel(mode){
      previewMode = mode;
      viewRgbBtn?.classList.toggle('active', mode === 'rgb');
      viewAlphaBtn?.classList.toggle('active', mode === 'a');
      logBoth('channel: ' + (mode === 'rgb' ? 'RGB' : 'A'));
      renderPreview();
    }

    function toggleCompare(){
      if (!prevImgData) { return; }
      compareMode = !compareMode;
      compareBtn?.classList.toggle('active', compareMode);
      logBoth('compare: ' + (compareMode ? 'B' : 'A'));
      renderPreview();
    }

    viewRgbBtn?.addEventListener('click', () => setChannel('rgb'));
    viewAlphaBtn?.addEventListener('click', () => setChannel('a'));
    compareBtn?.addEventListener('click', toggleCompare);

    document.addEventListener('keydown', (e) => {
      if (['INPUT','SELECT','TEXTAREA'].includes(e.target?.tagName)) return;
      if (e.key === 'j' || e.key === 'ArrowLeft') setChannel('rgb');
      else if (e.key === 'l' || e.key === 'ArrowRight') setChannel('a');
      else if (e.key === 'k' || e.key === 'ArrowUp' || e.key === 'ArrowDown') toggleCompare();
    });

    if (openBtn) openBtn.addEventListener('click', openExr);

    // Transform 一覧ロード（Resolve風）
    const transformEl = getEl('transform');
    const swapTransformBtn = null; // Swap UI廃止
    let transforms = [];
    (async () => {
      try {
        if (!(await ensureTauriReady())) return;
        transforms = await invoke('transform_presets');
        if (transformEl && Array.isArray(transforms)) {
          const byGroup = {};
          for (const t of transforms) { const g = t.group || 'General'; if (!byGroup[g]) byGroup[g] = []; byGroup[g].push(t); }
          // 先頭に NonTransform を追加（変換なしプレビュー）
          const nonTransformGroup = `<optgroup label="Bypass"><option value="NonTransform">NonTransform</option></optgroup>`;
          transformEl.innerHTML = nonTransformGroup +
            Object.keys(byGroup)
              .map(g => `<optgroup label="${g}">` + byGroup[g].map(t => `<option value="${t.label}">${t.label}</option>`).join('') + `</optgroup>`)
              .join('');
          // Settings側のDefault Transformも同じ一覧を流用
          if (defaultTransformEl) {
            const nonTransformGroup2 = `<optgroup label="Bypass"><option value="NonTransform">NonTransform</option></optgroup>`;
            defaultTransformEl.innerHTML = nonTransformGroup2 +
              Object.keys(byGroup)
                .map(g => `<optgroup label="${g}">` + byGroup[g].map(t => `<option value="${t.label}">${t.label}</option>`).join('') + `</optgroup>`)
                .join('');
          }
          // 既定Transformを読み込んで選択
          try {
            const def = await invoke('get_default_transform');
            if (def && transformEl.querySelector(`option[value="${def}"]`)) {
              transformEl.value = def;
              if (defaultTransformEl) defaultTransformEl.value = def;
            } else {
              // 初期値: 最初の項目
              const first = transformEl.querySelector('option');
              if (first) {
                transformEl.value = first.value;
                if (defaultTransformEl) defaultTransformEl.value = first.value;
              }
            }
          } catch (_) {}
          await logBoth('Transform一覧をロードしました');
          // 既定選択を即適用
          try { transformEl.dispatchEvent(new Event('change')); } catch (_) {}
        }
      } catch (e) { await logBoth('Transform読込失敗: ' + e); }
    })();
    if (transformEl) transformEl.addEventListener('change', async () => {
      // 特別項目: NonTransform（変換なし）
      if (transformEl.value === 'NonTransform') {
        try {
          if (!(await ensureTauriReady())) return;
          try { await invoke('clear_lut'); } catch (_) { /* ignore */ }
          useStateLutEnabled = false;
          updateLater();
          await logBoth('Transform適用: NonTransform（適用なし）');
        } catch (e) { appendLog('NonTransform 適用失敗: ' + e); }
        return;
      }
      const tsel = transforms.find(x => x.label === transformEl.value);
      if (!tsel) { await logBoth('Transform未選択'); return; }
      try {
        if (!(await ensureTauriReady())) return;
        const size = tsel.size || 33;
        await invoke('set_lut_3d', { srcSpace: tsel.src_space, srcTf: tsel.src_tf, dstSpace: tsel.dst_space, dstTf: tsel.dst_tf, size: Math.max(17, Math.min(65, size)), clipMode: 'clip' });
        useStateLutEnabled = true;
        updateLater();
        await logBoth('Transform適用: ' + tsel.label);
      } catch (e) { appendLog('Transform適用失敗: ' + e); }
    });

    // Settings: 既定Transformの保存
    if (defaultTransformEl) defaultTransformEl.addEventListener('change', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const label = defaultTransformEl.value;
        await invoke('set_default_transform', { label });
        await logBoth('Default Transform 保存: ' + label);
      } catch (e) { appendLog('Default Transform 保存失敗: ' + e); }
    });

    // load config
    (async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const [ms, pct] = await invoke('get_progress_config');
        if (progIntervalEl) progIntervalEl.value = ms;
        if (progThreshEl) progThreshEl.value = pct;
        const allow = await invoke('get_log_permission');
        if (logConsentEl) logConsentEl.checked = allow;
      } catch (_) {}
    })();

    logConsentEl?.addEventListener('change', async () => {
      try { if (invoke || await ensureTauriReady()) { await invoke('set_log_permission', { allow: !!logConsentEl.checked }); } } catch (_) {}
    });

    const saveProgress = debounce(async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const ms = parseInt(progIntervalEl?.value ?? '100', 10) || 0;
        const pct = parseFloat(progThreshEl?.value ?? '0.5') || 0;
        await invoke('set_progress_config', { intervalMs: ms, pctThreshold: pct });
      } catch (_) {}
    }, 500);
    progIntervalEl?.addEventListener('input', saveProgress);
    progThreshEl?.addEventListener('input', saveProgress);
    progIntervalResetBtn?.addEventListener('click', () => { if (progIntervalEl) { progIntervalEl.value = '100'; saveProgress(); } });
    progThreshResetBtn?.addEventListener('click', () => { if (progThreshEl) { progThreshEl.value = '0.5'; saveProgress(); } });

    if (browseBtn) browseBtn.addEventListener('click', async () => {
      try {
        const t = window.__TAURI__;
        const dialogOpen = t && (t.dialog && t.dialog.open) ? t.dialog.open : (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open ? t.tauri.dialog.open : null);
        if (dialogOpen) {
          const selected = await dialogOpen({ multiple: false, filters: [{ name: 'EXR', extensions: ['exr'] }] });
          if (selected) { pathEl.value = selected; await openExr(); }
        } else {
          const p = prompt('EXRファイルのパスを入力');
          if (p) { pathEl.value = p; await openExr(); }
        }
      } catch (e) { appendLog('ファイルダイアログ失敗: ' + e); }
    });

    // 属性テーブルは閲覧専用のため、追加・編集・削除は不可

    if (saveBtn) saveBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const t = window.__TAURI__;
        const dialogOpen = t && (t.dialog && t.dialog.open) ? t.dialog.open : (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open ? t.tauri.dialog.open : null);
        if (dialogOpen) {
          const selected = await dialogOpen({ multiple: true, filters: [{ name: 'EXR', extensions: ['exr'] }] });
          if (selected && (Array.isArray(selected) ? selected.length > 0 : true)) {
            const paths = Array.isArray(selected) ? selected : [selected];
            queueExportFiles(paths);
            return;
          }
        }
      } catch (e) { appendLog('ファイル選択失敗: ' + e); }
      const out = prompt('保存するPNGパスを入力:', 'preview.png');
      if (!out) return;
      try {
        await invoke('export_preview_png', { outPath: out });
        appendLog('PNG保存: ' + out);
        alert('保存しました: ' + out);
      } catch (e) { alert('保存に失敗: ' + e); }
    });

    if (cv) {
      cv.addEventListener('mousemove', async (ev) => {
        if (imgW === 0 || pipetteFixed) return;
        const rect = cv.getBoundingClientRect();
        const x = Math.floor((ev.clientX - rect.left));
        const y = Math.floor((ev.clientY - rect.top));
        try {
          if (!(invoke || await ensureTauriReady())) return;
          const [r,g,b,a] = await invoke('probe_pixel', { px: x, py: y });
          const readout = getEl('readout');
          if (readout) readout.textContent = `x:${x}, y:${y}  linear: R ${r.toFixed(6)}  G ${g.toFixed(6)}  B ${b.toFixed(6)}  A ${a.toFixed(6)}`;
        } catch (_) { /* ignore */ }
      });
      cv.addEventListener('click', async (ev) => {
        if (imgW === 0) return;
        const rect = cv.getBoundingClientRect();
        const x = Math.floor((ev.clientX - rect.left));
        const y = Math.floor((ev.clientY - rect.top));
        if (!pipetteFixed) {
          try {
            if (!(invoke || await ensureTauriReady())) return;
            const [r,g,b,a] = await invoke('probe_pixel', { px: x, py: y });
            const text = `x:${x}, y:${y}  linear: R ${r.toFixed(6)}  G ${g.toFixed(6)}  B ${b.toFixed(6)}  A ${a.toFixed(6)}`;
            const readout = getEl('readout');
            if (readout) readout.textContent = text;
            try { await navigator.clipboard.writeText(text); } catch (_) {}
            pipetteFixed = true;
          } catch (_) { /* ignore */ }
        } else {
          pipetteFixed = false;
        }
      });
    }

    // live update (debounced)
    async function updatePreview() {
      try {
        if (!(await ensureTauriReady())) return;
        const [w,h,b64] = await invoke('update_preview', {
          maxSize: MAX_PREVIEW,
          exposure: 0,
          gamma: 1.0,
          lutPath: null,
          useStateLut: useStateLutEnabled,
          highQuality: true
        });
        const img = new Image();
        img.onload = () => {
          const ctx = cv.getContext('2d');
          cv.width = w; cv.height = h;
          ctx.clearRect(0, 0, w, h);
          ctx.drawImage(img, 0, 0);
          imgW = w; imgH = h;
          prevImgData = currImgData;
          currImgData = ctx.getImageData(0,0,w,h);
          renderPreview();
        };
        img.src = 'data:image/png;base64,' + b64;
        await refreshScopes();
        showError('');
      } catch (e) {
        appendLog('update失敗: ' + e);
        showError('update失敗: ' + e);
      }
    }

    const updateLater = debounce(updatePreview, UPDATE_DEBOUNCE_MS);

    // LUT生成機能は削除

    // Transform適用ボタンは廃止（変更時に自動適用）

    /* Preset UI 廃止
    if (lutPreset) lutPreset.addEventListener('change', async () => {
      const val = lutPreset.value;
      if (!val) return;
      const [src, dst] = val.split('-');
      if (lutSrc) lutSrc.value = src;
      if (lutDst) lutDst.value = dst;
      try {
        if (!(await ensureTauriReady())) return;
        const size = ((src === 'linear' || src === 'srgb') && (dst === 'linear' || dst === 'srgb'))
          ? (parseInt(lutSize?.value ?? '1024',10) || 1024)
          : (parseInt(lutSize?.value ?? '33',10) || 33);
        if ((src === 'linear' || src === 'srgb') && (dst === 'linear' || dst === 'srgb')) {
          await invoke('set_lut_1d', { src, dst, size });
        } else {
          const dstTf = (dst === 'srgb') ? 'srgb' : (dst === 'g22' ? 'g22' : (dst === 'g24' ? 'g24' : 'linear'));
          await invoke('set_lut_3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: dstTf, size: Math.max(17, Math.min(65, size)), clipMode: 'clip' });
        }
        if (useStateLut) useStateLut.checked = true;
        updateLater();
        appendLog('Preset適用: ' + src + ' -> ' + dst);
      } catch (e) { appendLog('Preset適用失敗: ' + e); }
    });

    // LUT解除は廃止（常時メモリLUT使用）

    if (lutPreset) lutPreset.dispatchEvent(new Event('change'));
    */

    // --- OCIO settings ---
    const ocioDiv = getEl('ocio-settings');
    const ocioDisplay = getEl('ocio-display');
    const ocioView = getEl('ocio-view');
    const applyOcio = getEl('apply-ocio');

    async function refreshOcioViews() {
      if (!ocioDisplay || !ocioView) return;
      try {
        if (!(await ensureTauriReady())) return;
        const views = await invoke('ocio_views', { display: ocioDisplay.value });
        ocioView.innerHTML = (views || []).map(v => `<option>${v}</option>`).join('');
      } catch (_) {}
    }

    async function initOcio() {
      if (!ocioDiv || !ocioDisplay || !ocioView) return;
      try {
        if (!(await ensureTauriReady())) return;
        const displays = await invoke('ocio_displays');
        if (!Array.isArray(displays) || displays.length === 0) return;
        ocioDiv.style.display = 'block';
        ocioDisplay.innerHTML = displays.map(d => `<option>${d}</option>`).join('');
        const sel = await invoke('ocio_selection');
        if (Array.isArray(sel)) {
          if (sel[0]) ocioDisplay.value = sel[0];
          await refreshOcioViews();
          if (sel[1]) ocioView.value = sel[1];
        } else {
          await refreshOcioViews();
        }
      } catch (_) {}
    }

    if (ocioDisplay) ocioDisplay.addEventListener('change', refreshOcioViews);
    if (applyOcio) applyOcio.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        await invoke('set_ocio_display_view', { display: ocioDisplay?.value, view: ocioView?.value });
        updateLater();
      } catch (e) { appendLog('OCIO設定失敗: ' + e); }
    });

    initOcio();

    // 早期にTauri注入が完了するケース向け
    ensureTauriReady(2000);
    appendLog('UI ready');

    // ログ表示/消去
    const showLogBtn = getEl('showlog');
    const clearLogBtn = getEl('clearlog');
    if (showLogBtn) showLogBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const text = await invoke('read_log');
        const box = getEl('logbox');
        if (box) { box.textContent = text || '<empty>'; box.scrollTop = box.scrollHeight; }
      } catch (e) { appendLog('ログ取得失敗: ' + e); }
    });
    if (clearLogBtn) clearLogBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        await invoke('clear_log');
        const box = getEl('logbox');
        if (box) box.textContent = '';
        appendLog('ログを消去しました');
      } catch (e) { appendLog('ログ消去失敗: ' + e); }
    });
  });
    // --- Side panel tabs ---
    const sideBtnColor = getEl('side-tab-btn-color');
    const sideBtnInfo = getEl('side-tab-btn-info');
    const sideBtnTransform = getEl('side-tab-btn-transform');
    const sideBtnExport = getEl('side-tab-btn-export');
    const sideColor = getEl('side-tab-color');
    const sideInfo = getEl('side-tab-info');
    const sideTransform = getEl('side-tab-transform');
    const sideExport = getEl('side-tab-export');
    function activateSide(tab){
      if (!sideColor || !sideInfo || !sideTransform || !sideExport) return;
      sideColor.style.display = (tab === 'color') ? 'block' : 'none';
      sideInfo.style.display = (tab === 'info') ? 'block' : 'none';
      sideTransform.style.display = (tab === 'transform') ? 'block' : 'none';
      sideExport.style.display = (tab === 'export') ? 'block' : 'none';
      sideBtnColor?.classList.toggle('active', tab === 'color');
      sideBtnInfo?.classList.toggle('active', tab === 'info');
      sideBtnTransform?.classList.toggle('active', tab === 'transform');
      sideBtnExport?.classList.toggle('active', tab === 'export');
    }
    sideBtnColor?.addEventListener('click', () => { logBoth('side-tab: color'); activateSide('color'); });
    sideBtnInfo?.addEventListener('click', () => { logBoth('side-tab: info'); activateSide('info'); });
    sideBtnTransform?.addEventListener('click', () => { logBoth('side-tab: transform'); activateSide('transform'); });
    sideBtnExport?.addEventListener('click', () => { logBoth('side-tab: export'); activateSide('export'); });

    // --- Export controls ---
    const seqDirEl = getEl('seq-dir');
    const browseSeqBtn = getEl('browse-seq');
    const proresFpsEl = getEl('prores-fps');
    const proresFpsResetBtn = getEl('prores-fps-reset');
    const proresCsEl = getEl('prores-colorspace');
    const proresProfileEl = getEl('prores-profile');
    const proresMaxEl = getEl('prores-max');
    const proresMaxResetBtn = getEl('prores-max-reset');
    // Exposure input removed for ProRes
    const proresTfEl = getEl('prores-tf');
    const proresQualityEl = getEl('prores-quality');
    const proresOutEl = getEl('prores-out');
    const browseProresOutBtn = getEl('browse-prores-out');
    const proresProg = getEl('prores-progress');

    // Folder browse (EXR sequence)
    if (browseSeqBtn) browseSeqBtn.addEventListener('click', async () => {
      await logBoth('browse-seq clicked');
      try {
        if (!(await ensureTauriReady())) return;
        const t = window.__TAURI__;
        const dialogOpen = (t && t.dialog && t.dialog.open) || (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open) || null;
        if (dialogOpen) {
          const selected = await dialogOpen({ multiple: false, directory: true, defaultPath: seqDirEl?.value || undefined });
          if (selected && seqDirEl) seqDirEl.value = Array.isArray(selected) ? selected[0] : selected;
          await logBoth(`フォルダ選択: ${seqDirEl?.value || ''}`);
        } else {
          const p = prompt('EXR連番フォルダのパスを入力'); if (p && seqDirEl) seqDirEl.value = p;
          await logBoth(`フォルダ入力: ${seqDirEl?.value || ''}`);
        }
      } catch (e) { appendLog('フォルダダイアログ失敗: ' + e); }
    });

    // ProRes output browse
    if (browseProresOutBtn) browseProresOutBtn.addEventListener('click', async () => {
      await logBoth('browse-prores-out clicked');
      try {
        if (!(await ensureTauriReady())) return;
        const t = window.__TAURI__;
        const saveDlg = (t && t.dialog && t.dialog.save) || (t && t.tauri && t.tauri.dialog && t.tauri.dialog.save) || null;
        if (saveDlg) {
          const sel = await saveDlg({ filters: [{ name: 'ProRes MOV', extensions: ['mov'] }], defaultPath: proresOutEl?.value || undefined });
          if (sel && proresOutEl) proresOutEl.value = sel;
          await logBoth(`出力選択: ${proresOutEl?.value || ''}`);
        } else {
          const p = prompt('出力MOVのパスを入力 (.mov)'); if (p && proresOutEl) proresOutEl.value = p;
          await logBoth(`出力入力: ${proresOutEl?.value || ''}`);
        }
      } catch (e) { appendLog('出力選択失敗: ' + e); }
    });

    // Export ProRes
    const exportProresBtn = getEl('export-prores');
    if (exportProresBtn) exportProresBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const dir = seqDirEl?.value?.trim(); const out = proresOutEl?.value?.trim();
        if (!dir) { alert('Sequence Folder を指定してください'); return; }
        if (!out) { alert('出力MOVのパスを指定してください'); return; }
        const fps = parseFloat(proresFpsEl?.value ?? '24') || 24;
        const colorspace = (proresCsEl?.value || 'linear:srgb');
        const profile = (proresProfileEl?.value || '422hq');
        const maxSize = parseInt(proresMaxEl?.value ?? '2048', 10) || 2048;
        const exposure = 0;
        const gamma = ((()=>{ const v=(proresTfEl?.value||'g22'); if (v==='g24') return 2.4; if (v==='linear') return 1.0; return 2.2; })());
        const quality = (proresQualityEl?.value || 'High');
        await logBoth(`export_prores: dir=${dir} out=${out} fps=${fps} cs=${colorspace} profile=${profile}`);

        // listen progress
        const t = window.__TAURI__;
        if (t && t.event && t.event.listen && proresProg) {
          proresProg.style.display = 'block'; proresProg.value = 0;
          const unlisten = await t.event.listen('video-progress', (e) => { try { proresProg.value = e.payload; } catch(_){} });
          try {
            await invoke('export_prores', { dir, fps, colorspace, out, profile, maxSize, exposure, gamma, quality });
            appendLog('ProRes出力完了: ' + out);
            alert('出力完了: ' + out);
          } finally { unlisten(); proresProg.style.display = 'none'; }
        } else {
          await invoke('export_prores', { dir, fps, colorspace, out, profile, maxSize, exposure, gamma, quality });
          alert('出力完了: ' + out);
        }
      } catch (e) { appendLog('ProRes出力失敗: ' + e); alert('ProRes出力失敗: ' + e); }
    });
})();
    proresFpsResetBtn?.addEventListener('click', () => { if (proresFpsEl) proresFpsEl.value = '24'; });
    proresMaxResetBtn?.addEventListener('click', () => { if (proresMaxEl) proresMaxEl.value = '2048'; });
