(() => {
  let invoke = null; // 解決済みの invoke（nullなら未解決）
  let imgW = 0, imgH = 0;
  let useStateLutEnabled = false; // LUT in-memory 使用フラグ
  let pipetteFixed = false; // スポイト固定

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
        if (typeof drawHistogram === 'function' && typeof stats !== 'undefined') {
          drawHistogram(stats);
        }
      };
      img.src = 'data:image/png;base64,' + b64;
      await loadMetadata(path);
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
    const lutPreset = getEl('lut-preset');
    const makeLutBtn = getEl('make-lut');
    const applyPresetBtn = getEl('apply-preset');
    const clearLutBtn = getEl('clear-lut');
    const useStateLut = getEl('use-state-lut');
    const addAttrBtn = getEl('add-attr');
    attrTable = getEl('attr-table');

    useStateLutEnabled = !!useStateLut?.checked;

    if (openBtn) openBtn.addEventListener('click', openExr);

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
          if (typeof drawHistogram === 'function' && typeof stats !== 'undefined') {
            drawHistogram(stats);
          }
        };
        img.src = 'data:image/png;base64,' + b64;
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
          await invoke('make_lut3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)), outPath: out });
          appendLog('3D LUT生成: ' + out);
        }
      } catch (e) { appendLog('LUT生成失敗: ' + e); }
    });

    if (applyPresetBtn) applyPresetBtn.addEventListener('click', async () => {
      try {
        if (!(await ensureTauriReady())) return;
        const src = (lutSrc?.value || 'linear').toLowerCase();
        const dst = (lutDst?.value || 'srgb').toLowerCase();
        const size = parseInt(lutSize?.value ?? '33',10) || 33;
        const clip = (lutClip?.value || 'clip').toLowerCase();
        if (src === 'linear' || src === 'srgb') {
          await invoke('set_lut_1d', { src, dst, size });
        } else {
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
        const size = parseInt(lutSize?.value ?? '33',10) || 33;
        if (src === 'linear' || src === 'srgb') {
          await invoke('set_lut_1d', { src, dst, size });
        } else {
          await invoke('set_lut_3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)) });
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
})();
