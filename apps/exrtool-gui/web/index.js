(() => {
  let invoke = null; // 解決済みの invoke（nullなら未解決）
  let imgW = 0, imgH = 0;

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

  function drawHistogram(stats) {
    const cv = getEl('hist');
    if (!cv || !stats) return;
    const ctx = cv.getContext('2d');
    const bins = stats.hist_r.length;
    cv.width = bins;
    cv.height = 100;
    ctx.clearRect(0, 0, cv.width, cv.height);
    const max = Math.max(1, ...stats.hist_r, ...stats.hist_g, ...stats.hist_b);
    for (let i = 0; i < bins; i++) {
      const r = stats.hist_r[i] / max * cv.height;
      const g = stats.hist_g[i] / max * cv.height;
      const b = stats.hist_b[i] / max * cv.height;
      ctx.fillStyle = 'rgba(255,0,0,0.5)';
      ctx.fillRect(i, cv.height - r, 1, r);
      ctx.fillStyle = 'rgba(0,255,0,0.5)';
      ctx.fillRect(i, cv.height - g, 1, g);
      ctx.fillStyle = 'rgba(0,0,255,0.5)';
      ctx.fillRect(i, cv.height - b, 1, b);
    }
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
    const maxEl = getEl('max');
    const expEl = getEl('exp');
    const gammaEl = getEl('gamma');
    const cv = getEl('cv');
    const info = getEl('info');
    if (!pathEl || !cv || !info) return;
    const ctx = cv.getContext('2d');

    const path = pathEl.value.trim();
    const lutPath = lutEl ? (lutEl.value.trim() || null) : null;
    try {
      if (!(await ensureTauriReady())) throw new Error('Tauri API が利用できません');
      const [w, h, b64, stats] = await invoke('open_exr', {
        path,
        maxSize: parseInt(maxEl?.value ?? '2048', 10) || 2048,
        exposure: parseFloat(expEl?.value ?? '0'),
        gamma: parseFloat(gammaEl?.value ?? '2.2'),
        lutPath
      });
      const img = new Image();
      img.onload = () => {
        imgW = w; imgH = h;
        cv.width = w; cv.height = h;
        ctx.clearRect(0,0,w,h);
        ctx.drawImage(img, 0, 0);
        info.textContent = `preview: ${w}x${h}`;
        appendLog(`open ok: ${w}x${h}`);
        drawHistogram(stats);
      };
      img.src = 'data:image/png;base64,' + b64;
    } catch (e) {
      appendLog('読み込み失敗: ' + e);
      alert('読み込み失敗: ' + e);
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
    const lutSrc = getEl('lut-src');
    const lutDst = getEl('lut-dst');
    const lutSize = getEl('lut-size');
    const makeLutBtn = getEl('make-lut');
    const applyPresetBtn = getEl('apply-preset');
    const clearLutBtn = getEl('clear-lut');
    const useStateLut = getEl('use-state-lut');

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

    if (cv) cv.addEventListener('mousemove', async (ev) => {
      if (imgW === 0) return;
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

    // Exposure/Gamma live update (debounced)
    let timer = null;
    const scheduleUpdate = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(async () => {
        try {
          if (!(await ensureTauriReady())) return;
          const maxEl = getEl('max');
          const lutEl = getEl('lut');
          const [w,h,b64,stats] = await invoke('update_preview', {
            maxSize: parseInt(maxEl?.value ?? '2048',10) || 2048,
            exposure: parseFloat(expEl?.value ?? '0'),
            gamma: parseFloat(gammaEl?.value ?? '2.2'),
            lutPath: (lutEl && lutEl.value.trim() && !(useStateLut?.checked)) ? lutEl.value.trim() : null,
            useStateLut: !!(useStateLut?.checked),
          });
          const img = new Image();
          const info = getEl('info');
          img.onload = () => {
            const ctx = cv.getContext('2d');
            cv.width = w; cv.height = h;
            ctx.clearRect(0,0,w,h);
            ctx.drawImage(img, 0, 0);
            if (info) info.textContent = `preview: ${w}x${h}`;
            drawHistogram(stats);
          };
          img.src = 'data:image/png;base64,' + b64;
        } catch (e) { appendLog('update失敗: ' + e); }
      }, 120);
    };
    if (expEl) expEl.addEventListener('input', scheduleUpdate);
    if (gammaEl) gammaEl.addEventListener('input', scheduleUpdate);
    if (useStateLut) useStateLut.addEventListener('change', scheduleUpdate);

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
        if (src === 'linear' || src === 'srgb') {
          await invoke('set_lut_1d', { src, dst, size });
        } else {
          await invoke('set_lut_3d', { srcSpace: src, srcTf: 'linear', dstSpace: dst, dstTf: 'srgb', size: Math.max(17, Math.min(65, size)) });
        }
        if (useStateLut) useStateLut.checked = true;
        scheduleUpdate();
        appendLog('Preset適用: ' + src + ' -> ' + dst);
      } catch (e) { appendLog('Preset適用失敗: ' + e); }
    });

    if (clearLutBtn) clearLutBtn.addEventListener('click', async () => {
      try { if (!(await ensureTauriReady())) return; await invoke('clear_lut'); if (useStateLut) useStateLut.checked = false; scheduleUpdate(); appendLog('LUT解除'); } catch (e) { appendLog('解除失敗: ' + e); }
    });

    // 早期にTauri注入が完了するケース向け
    ensureTauriReady(2000);
    appendLog('UI ready');
  });
})();
