(() => {
  let invoke = null; // 解決済みの invoke（nullなら未解決）
  let imgW = 0, imgH = 0;
  let useStateLutEnabled = false; // LUT in-memory 使用フラグ
  let pipetteFixed = false; // スポイト固定
  let stats = null;
  let waveform = null;
  let scopeChannel = 'rgb';
  let scopeScale = 1;

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

  async function showSeqSummary(success, failure, dryRun) {
    const total = success + failure;
    const msg = dryRun
      ? `対象ファイル: ${total}`
      : `連番処理完了: 成功 ${success} 件 / 失敗 ${failure} 件 (全${total}件)`;
    await logBoth(`seq_fps result: success=${success} failure=${failure} total=${total}${dryRun ? ' (dry-run)' : ''}`);
    alert(msg);
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

  async function openExr() {
    const pathEl = getEl('path');
    const lutEl = getEl('lut');
    const ocioEl = getEl('ocio');
    const maxEl = getEl('max');
    const expEl = getEl('exp');
    const gammaEl = getEl('gamma');
    const hqEl = getEl('hq');
    const cv = getEl('cv');
    const info = getEl('info');
    if (!pathEl || !cv || !info) return;
    const ctx = cv.getContext('2d');

    const path = pathEl.value.trim();
    const lutPath = (!useStateLutEnabled && lutEl) ? (lutEl.value.trim() || null) : null;
    try {
      if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
      const t = window.__TAURI__;
      const listen = t && (t.event && t.event.listen ? t.event.listen : (t.tauri && t.tauri.event && t.tauri.event.listen ? t.tauri.event.listen : null));
      if (listen) { unlisten = await listen('open-progress', e => { progEl.value = e.payload; }); }
      const [w, h, b64] = await invoke('open_exr', {
        path,
        maxSize: parseInt(maxEl?.value ?? '2048', 10) || 2048,
        exposure: parseFloat(expEl?.value ?? '0'),
        gamma: parseFloat(gammaEl?.value ?? '2.2'),
        lutPath,
        highQuality: !!(hqEl?.checked)
      });
      const img = new Image();
      img.onload = () => {
        imgW = w; imgH = h;
        cv.width = w; cv.height = h;
        ctx.clearRect(0,0,w,h);
        ctx.drawImage(img, 0, 0);
        info.textContent = `preview: ${w}x${h}`;
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
      originalAttrs = new Map(entries.map(([k, v]) => [String(k), String(v)]));
      tbody.innerHTML = '';
      for (const [name, value] of originalAttrs) {
        const tr = document.createElement('tr');
        tr.innerHTML = `<td class="name" contenteditable="true"></td><td class="value" contenteditable="true"></td><td><button class="del">削除</button></td>`;
        tr.querySelector('.name').textContent = name;
        tr.querySelector('.value').textContent = value;
        tr.dataset.originalName = name;
        tr.dataset.originalValue = value;
        tbody.appendChild(tr);
      }
    } catch (e) { appendLog('metadata読み込み失敗: ' + e); }
  }

  function markDiff(tr) {
    if (tr.classList.contains('added')) return;
    const name = tr.querySelector('.name')?.textContent || '';
    const value = tr.querySelector('.value')?.textContent || '';
    const on = tr.dataset.originalName || '';
    const ov = tr.dataset.originalValue || '';
    if (name !== on || value !== ov) {
      tr.classList.add('modified');
    } else {
      tr.classList.remove('modified');
    }
  }

  document.addEventListener('DOMContentLoaded', () => {
    // Tabs
    const tabBtnPreview = document.getElementById('tab-btn-preview');
    const tabBtnVideo = document.getElementById('tab-btn-video');
    const tabBtnSettings = document.getElementById('tab-btn-settings');
    const tabPreview = document.getElementById('tab-preview');
    const tabVideo = document.getElementById('tab-video');
    const tabSettings = document.getElementById('tab-settings');
    function activate(tab){
      if (!tabPreview || !tabVideo || !tabSettings) return;
      tabPreview.style.display = (tab === 'preview') ? 'block' : 'none';
      tabVideo.style.display = (tab === 'video') ? 'block' : 'none';
      tabSettings.style.display = (tab === 'settings') ? 'block' : 'none';
      tabBtnPreview?.classList.toggle('active', tab === 'preview');
      tabBtnVideo?.classList.toggle('active', tab === 'video');
      tabBtnSettings?.classList.toggle('active', tab === 'settings');
    }
    tabBtnPreview?.addEventListener('click', ()=>{ logBoth('tab: preview'); activate('preview'); });
    tabBtnVideo?.addEventListener('click', ()=>{ logBoth('tab: video'); activate('video'); });
    tabBtnSettings?.addEventListener('click', ()=>{ logBoth('tab: settings'); activate('settings'); });
    logBoth('boot: video controls present? browse-seq=' + (!!document.getElementById('browse-seq')) + ', browse-prores-out=' + (!!document.getElementById('browse-prores-out')));
    const openBtn = getEl('open');
    const browseBtn = getEl('browse');
    const saveBtn = getEl('save');
    const cv = getEl('cv');
    const pathEl = getEl('path');
    const expEl = getEl('exp');
    const gammaEl = getEl('gamma');
    const hqEl = getEl('hq');
    const lutSrc = getEl('lut-src');
    const lutDst = getEl('lut-dst');
    const lutSize = getEl('lut-size');
    const lutClip = getEl('lut-clip');
    const lutPreset = getEl('lut-preset');
    const makeLutBtn = getEl('make-lut');
    const cancelLutBtn = getEl('cancel-lut');
    const lutProg = getEl('lut-progress');
    const applyPresetBtn = getEl('apply-preset');
    const clearLutBtn = getEl('clear-lut');
    const useStateLut = getEl('use-state-lut');
    const addAttrBtn = getEl('add-attr');
    const progIntervalEl = getEl('progress-interval');
    const progThreshEl = getEl('progress-threshold');
    const logConsentEl = getEl('log-consent');
    attrTable = getEl('attr-table');
    const scopeChannelEl = getEl('scope-channel');
    const scopeScaleEl = getEl('scope-scale');

    useStateLutEnabled = !!useStateLut?.checked;
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

    if (openBtn) openBtn.addEventListener('click', openExr);

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

    if (browseBtn) browseBtn.addEventListener('click', async () => {
      try {
        const t = window.__TAURI__;
        const dialogOpen = t && (t.dialog && t.dialog.open) ? t.dialog.open : (t && t.tauri && t.tauri.dialog && t.tauri.dialog.open ? t.tauri.dialog.open : null);
        if (dialogOpen) {
          const selected = await dialogOpen({ multiple: false, filters: [{ name: 'EXR', extensions: ['exr'] }] });
          if (selected) { pathEl.value = selected; }
        } else {
          const p = prompt('EXRファイルのパスを入力');
          if (p) pathEl.value = p;
        }
      } catch (e) { appendLog('ファイルダイアログ失敗: ' + e); }
    });

    if (addAttrBtn) addAttrBtn.addEventListener('click', () => {
      const tbody = attrTable?.querySelector('tbody');
      if (!tbody) return;
      const tr = document.createElement('tr');
      tr.classList.add('added');
      tr.innerHTML = `<td class="name" contenteditable="true"></td><td class="value" contenteditable="true"></td><td><button class="del">削除</button></td>`;
      tbody.appendChild(tr);
    });

    if (attrTable) {
      attrTable.addEventListener('input', (e) => {
        const tr = e.target.closest('tr');
        if (!tr) return;
        if (tr.classList.contains('deleted')) tr.classList.remove('deleted');
        markDiff(tr);
      });
      attrTable.addEventListener('click', (e) => {
        if (e.target.classList.contains('del')) {
          const tr = e.target.closest('tr');
          if (!tr) return;
          if (tr.classList.contains('added')) {
            tr.remove();
          } else {
            tr.classList.toggle('deleted');
            if (tr.classList.contains('deleted')) tr.classList.remove('modified');
          }
        }
      });
    }

    if (saveBtn) saveBtn.addEventListener('click', async () => {
      const out = prompt('保存するPNGパスを入力:', 'preview.png');
      if (!out) return;
      try {
        if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
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

    // Exposure/Gamma live update (debounced)
    async function updatePreview() {
      try {
        if (!(await ensureTauriReady())) return;
        const maxEl = getEl('max');
        const lutEl = getEl('lut');
        const hqEl = getEl('hq');
        const [w,h,b64] = await invoke('update_preview', {
          maxSize: parseInt(maxEl?.value ?? '2048',10) || 2048,
          exposure: parseFloat(expEl?.value ?? '0'),
          gamma: parseFloat(gammaEl?.value ?? '2.2'),
          lutPath: (lutEl && lutEl.value.trim() && !useStateLutEnabled) ? lutEl.value.trim() : null,
          useStateLut: useStateLutEnabled,
          highQuality: !!(hqEl?.checked)
        });
        const img = new Image();
        const info = getEl('info');
        img.onload = () => {
          const ctx = cv.getContext('2d');
          cv.width = w; cv.height = h;
          ctx.clearRect(0, 0, w, h);
          ctx.drawImage(img, 0, 0);
          if (info) info.textContent = `preview: ${w}x${h}`;
        };
        img.src = 'data:image/png;base64,' + b64;
        await refreshScopes();
        showError('');
      } catch (e) {
        appendLog('update失敗: ' + e);
        showError('update失敗: ' + e);
      }
    }

    const updateLater = debounce(updatePreview, 120);
    if (expEl) expEl.addEventListener('input', updateLater);
    if (gammaEl) gammaEl.addEventListener('input', updateLater);
    if (useStateLut) useStateLut.addEventListener('change', () => {
      useStateLutEnabled = !!useStateLut.checked;
      updateLater();
    });

    if (makeLutBtn) makeLutBtn.addEventListener('click', async () => {
      try {
        const out = prompt('生成する .cube の保存先パス:', 'linear_to_srgb.cube');
        if (!out) return;
        if (!(await ensureTauriReady())) return;
        const src = (lutSrc?.value || 'linear').toLowerCase();
        const dst = (lutDst?.value || 'srgb').toLowerCase();
        const size = parseInt(lutSize?.value ?? '1024',10) || 1024;
        if (src === 'linear' || src === 'srgb') {
          // 1D LUT
          await invoke('make_lut', { src, dst, size, outPath: out });
          appendLog('1D LUT生成: ' + out);
        } else {
          // 3D LUT (色域+トーン変換)。src/dstを primaries として扱い、
          // トーンは src: linear, dst: srgb を既定とする。
          const t = window.__TAURI__;
          if (t && t.event && t.event.listen && lutProg && cancelLutBtn) {
            lutProg.style.display = 'inline';
            lutProg.value = 0;
            cancelLutBtn.style.display = 'inline';
            const unlisten = await t.event.listen('lut-progress', e => { try { lutProg.value = e.payload; } catch(_){} });
            const cancelHandler = () => { try { t.event.emit('lut-cancel'); } catch(_){} };
            cancelLutBtn.addEventListener('click', cancelHandler);
            try {
              await invoke('make_lut3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)), outPath: out });
              appendLog('3D LUT生成: ' + out);
            } catch (e) {
              if (String(e).includes('cancelled')) {
                appendLog('LUT生成キャンセル');
              } else {
                appendLog('LUT生成失敗: ' + e);
              }
            } finally {
              unlisten();
              cancelLutBtn.removeEventListener('click', cancelHandler);
              lutProg.style.display = 'none';
              cancelLutBtn.style.display = 'none';
            }
          } else {
            await invoke('make_lut3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)), outPath: out });
            appendLog('3D LUT生成: ' + out);
          }
        }
      } catch (e) { appendLog('LUT生成失敗: ' + e); }
    });

    if (applyPresetBtn) applyPresetBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const src = (lutSrc?.value || 'linear').toLowerCase();
        const dst = (lutDst?.value || 'srgb').toLowerCase();
        const size = ((src === 'linear' || src === 'srgb') && (dst === 'linear' || dst === 'srgb'))
          ? (parseInt(lutSize?.value ?? '1024',10) || 1024)
          : (parseInt(lutSize?.value ?? '33',10) || 33);
        const clip = (lutClip?.value || 'clip').toLowerCase();
        if ((src === 'linear' || src === 'srgb') && (dst === 'linear' || dst === 'srgb')) {
          await invoke('set_lut_1d', { src, dst, size });
        } else {
          // TauriはRustの`clip_mode`引数に対してcamelCaseキー`clipMode`を受け取る
          await invoke('set_lut_3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)), clipMode: clip });
        }
        if (useStateLut) useStateLut.checked = true;
        useStateLutEnabled = true;
        updateLater();
        appendLog('Preset適用: ' + src + ' -> ' + dst);
      } catch (e) { appendLog('Preset適用失敗: ' + e); }
    });

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
          await invoke('set_lut_3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)), clipMode: (lutClip?.value || 'clip').toLowerCase() });
        }
        if (useStateLut) useStateLut.checked = true;
        updateLater();
        appendLog('Preset適用: ' + src + ' -> ' + dst);
      } catch (e) { appendLog('Preset適用失敗: ' + e); }
    });

    if (clearLutBtn) clearLutBtn.addEventListener('click', async () => {
      try { if (!(await ensureTauriReady())) return; await invoke('clear_lut'); if (useStateLut) useStateLut.checked = false; useStateLutEnabled = false; updateLater(); appendLog('LUT解除'); } catch (e) { appendLog('解除失敗: ' + e); }
    });

    if (lutPreset) lutPreset.dispatchEvent(new Event('change'));

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
    // --- Video tab controls ---
    const seqDirEl = getEl('seq-dir');
    const browseSeqBtn = getEl('browse-seq');
    const seqFpsEl = getEl('seq-fps');
    const seqAttrEl = getEl('seq-fps-attr');
    const seqRecursiveEl = getEl('seq-fps-recursive');
    const seqDryRunEl = getEl('seq-fps-dryrun');
    const applyFpsBtn = getEl('apply-fps');
    const cancelFpsBtn = getEl('cancel-fps');
    const seqProg = getEl('seq-progress');

    const proresFpsEl = getEl('prores-fps');
    const proresCsEl = getEl('prores-colorspace');
    const proresProfileEl = getEl('prores-profile');
    const proresMaxEl = getEl('prores-max');
    const proresExpEl = getEl('prores-exp');
    const proresGammaEl = getEl('prores-gamma');
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

    // Apply FPS to sequence (metadata write)
    if (applyFpsBtn) applyFpsBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const dir = seqDirEl?.value?.trim();
        if (!dir) { alert('Sequence Folder を指定してください'); return; }
        const fps = parseFloat(seqFpsEl?.value ?? '24') || 24;
        const attr = (seqAttrEl?.value || 'FramesPerSecond');
        const recursive = !!seqRecursiveEl?.checked;
        const dryRun = !!seqDryRunEl?.checked;
        await logBoth(`seq_fps 実行: dir=${dir} fps=${fps} attr=${attr} recursive=${recursive} dryRun=${dryRun}`);

        const t = window.__TAURI__;
        if (t && t.event && t.event.listen && seqProg) {
          seqProg.style.display = 'block'; seqProg.value = 0;
          if (cancelFpsBtn) cancelFpsBtn.style.display = 'inline';
          const unlisten = await t.event.listen('seq-progress', (e) => { try { seqProg.value = e.payload; } catch(_){} });
          const cancelHandler = async () => { try { await invoke('cancel_seq_fps'); } catch(_){} };
          if (cancelFpsBtn) cancelFpsBtn.addEventListener('click', cancelHandler);
          try {
            const count = await invoke('seq_fps', { dir, fps, attr, recursive, dryRun, backup: true });
            await logBoth(`seq_fps: ${dryRun ? 'dry-run ' : ''}${count} files${dryRun ? ' (no changes)' : ''}`);
            if (dryRun) alert(`対象ファイル: ${count}`); else alert(`更新ファイル: ${count}`);
          } catch (e) {
            if (String(e).includes('cancelled')) {
              await logBoth('seq_fps cancelled');
            } else {
              appendLog('seq_fps失敗: ' + e); alert('seq_fps失敗: ' + e);
            }
          } finally {
            unlisten();
            seqProg.style.display = 'none'; seqProg.value = 0;
            if (cancelFpsBtn) { cancelFpsBtn.removeEventListener('click', cancelHandler); cancelFpsBtn.style.display = 'none'; }
          }
        } else {
          const res = await invoke('seq_fps', { dir, fps, attr, recursive, dryRun, backup: true });
          await showSeqSummary(res.success, res.failure, dryRun);
        }
      } catch (e) { appendLog('seq_fps失敗: ' + e); alert('seq_fps失敗: ' + e); }
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
        const exposure = parseFloat(proresExpEl?.value ?? '0') || 0;
        const gamma = parseFloat(proresGammaEl?.value ?? '2.2') || 2.2;
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
