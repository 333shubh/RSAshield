/*
 * dashboard.js — the audit console. Talks only to the local rsashield server
 * (same origin). Every value from the server is written with textContent, never
 * parsed as HTML.
 */
(function () {
  'use strict';

  const $ = (sel) => document.querySelector(sel);

  // tiny element builder: h('div', { class: 'x', text: 'y', onclick: fn }, ...children)
  function h(tag, props, ...kids) {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) {
      if (v == null || v === false) continue;
      if (k === 'class') n.className = v;
      else if (k === 'text') n.textContent = v;
      else if (k.startsWith('on')) n.addEventListener(k.slice(2), v);
      else n.setAttribute(k, v === true ? '' : v);
    }
    for (const kid of kids.flat()) if (kid != null) n.append(kid.nodeType ? kid : String(kid));
    return n;
  }

  async function api(method, path, body) {
    const res = await fetch(path, { method, body, headers: body ? { 'Content-Type': 'text/plain' } : undefined });
    let data;
    try {
      data = await res.json();
    } catch {
      data = { error: `HTTP ${res.status}` };
    }
    if (!res.ok) {
      const err = new Error(data.error || `HTTP ${res.status}`);
      err.status = res.status;
      err.data = data;
      throw err;
    }
    return data;
  }

  /* ------------------------------------------------------------ vocabulary */

  const STATUS = {
    FACTOR_RECOVERED: { tone: 'crit', label: 'Factor recovered', icon: 'M5 5l6 6M11 5l-6 6' },
    DEFECT_SUSPECTED: { tone: 'warn', label: 'Defect suspected', icon: 'M8 4v5M8 11.5v.5' },
    NO_DEFECT_FOUND: { tone: 'info', label: 'No defect found', icon: 'M4.5 8h7' },
  };
  const VERDICT = { FactorRecovered: 'crit', DefectSuspected: 'warn', Advisory: 'adv', Clean: '' };
  const DETECTORS = {
    batch_gcd: 'Batch GCD',
    small_factor: 'Small factor',
    fermat: 'Fermat',
    wiener: 'Wiener',
    ecm: 'ECM',
    metadata: 'Metadata',
  };
  // per_class_recall is keyed by the Rust enum name
  const CLASSES = {
    SharedPrime: 'Shared prime',
    ClosePrime: 'Close primes',
    SmallFactor: 'Small factor',
    Unbalanced: 'Unbalanced',
    SmallD: 'Small d (Wiener)',
  };

  const words = (camel) => camel.replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase().replace(/^./, (c) => c.toUpperCase());
  const realModes = (modes) => (modes || []).filter((m) => m !== 'None');
  const detectorName = (d) => DETECTORS[d] || words(d.replace(/_/g, ' '));

  function badge(classification) {
    const s = STATUS[classification] || STATUS.NO_DEFECT_FOUND;
    const icon = h('span', {});
    icon.innerHTML = `<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="${s.icon}" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>`;
    return h('span', { class: `badge ${s.tone}` }, icon.firstChild, s.label);
  }

  /* -------------------------------------------------------------- server */

  const status = $('#status');
  let online = false;

  async function checkHealth() {
    try {
      const data = await api('GET', '/health');
      online = true;
      status.className = 'status ok';
      status.querySelector('.status-text').textContent = `Online · ${data.stored_keys} stored`;
      $('#offline').hidden = true;
    } catch {
      online = false;
      status.className = 'status down';
      status.querySelector('.status-text').textContent = 'Server offline';
      $('#offline').hidden = false;
    }
    document.querySelectorAll('#auditBtn, #batchBtn, #clearKeys, #bundleBtn').forEach((b) => (b.disabled = !online));
    return online;
  }

  /* ------------------------------------------------------------- samples */

  async function loadSamples() {
    const list = $('#samples');
    list.replaceChildren(h('span', { class: 'hint', text: 'Generating seeded samples…' }));
    try {
      const samples = await api('GET', '/api/samples');
      list.replaceChildren(
        ...samples.map((s) =>
          h(
            'button',
            {
              class: 'sample',
              type: 'button',
              title: s.description,
              onclick: () => {
                $('#keyInput').value = s.keys.join('\n');
                runAudit();
              },
            },
            h('strong', { text: s.label }),
            h('span', { text: `Expect: ${s.expect}` })
          )
        )
      );
    } catch {
      list.replaceChildren(h('span', { class: 'hint', text: 'Samples are available when the server is running.' }));
    }
  }

  /* --------------------------------------------------------------- audit */

  let results = [];

  async function runAudit() {
    const text = $('#keyInput').value;
    if (!text.trim()) {
      $('#keyInput').focus();
      return;
    }
    $('#auditBtn').disabled = true;
    $('#auditBusy').hidden = false;
    try {
      const data = await api('POST', '/api/audit', text);
      results = data.results;
      showResult(0, data.errors);
      refreshKeys();
    } catch (err) {
      showError(err);
    } finally {
      $('#auditBtn').disabled = !online;
      $('#auditBusy').hidden = true;
    }
  }

  function showResult(index, errors) {
    const { id, report } = results[index];
    const body = $('#reportBody');
    $('#reportEmpty').hidden = true;
    body.hidden = false;

    const nodes = [];
    if (results.length > 1) {
      nodes.push(
        h(
          'div',
          { class: 'switcher', role: 'group', 'aria-label': 'Audited keys' },
          results.map((r, i) =>
            h('button', { type: 'button', 'aria-pressed': String(i === index), onclick: () => showResult(i) }, `#${r.id} · ${STATUS[r.report.classification].label}`)
          )
        )
      );
    }
    nodes.push(renderReport(id, report));
    if (errors && errors.length) {
      nodes.push(h('p', { class: 'partial', text: `${errors.length} record(s) could not be parsed: ${errors.map((e) => e.error).join('; ')}` }));
    }
    body.replaceChildren(...nodes);
  }

  function renderReport(id, r) {
    const tone = (STATUS[r.classification] || STATUS.NO_DEFECT_FOUND).tone;
    const modes = realModes(r.failure_modes);

    const top = h(
      'div',
      { class: 'r-top' },
      h('div', {}, badge(r.classification), h('p', { class: 'r-key', text: r.key })),
      h(
        'div',
        { class: 'score', 'aria-label': `Risk score ${r.risk_score} of 100` },
        h('span', { class: 'big', text: r.risk_score }),
        h('span', { class: 'of', text: ' / 100 risk' }),
        h('div', { class: 'meter' }, h('i', { class: tone, style: `width:${Math.min(100, r.risk_score)}%` }))
      )
    );

    const facts = h(
      'dl',
      { class: 'facts' },
      h('div', { class: 'fact' }, h('dt', { text: 'Modulus' }), h('dd', { text: `${r.modulus_bits}-bit` })),
      h(
        'div',
        { class: 'fact' },
        h('dt', { text: 'Failure modes' }),
        h('dd', {}, modes.length ? h('div', { class: 'chips' }, modes.map((m) => h('span', { class: 'chip', text: words(m) }))) : 'None found')
      ),
      h('div', { class: 'fact' }, h('dt', { text: 'Stored as' }), h('dd', { text: `#${id}` }))
    );

    const blocks = [top, facts, h('p', { class: 'caveat', text: r.caveat })];

    blocks.push(
      h('div', { class: 'block' }, h('h3', { text: 'Remediation' }), h('ol', { class: 'remedy' }, r.remediation.map((t) => h('li', { text: t }))))
    );

    if (r.standards && r.standards.length) {
      blocks.push(
        h(
          'div',
          { class: 'block' },
          h('h3', { text: 'ICS standards references' }),
          h(
            'div',
            { class: 'table-wrap' },
            h(
              'table',
              {},
              h('thead', {}, h('tr', {}, h('th', { scope: 'col', text: 'Framework' }), h('th', { scope: 'col', text: 'Reference' }), h('th', { scope: 'col', text: 'Why it applies' }))),
              h('tbody', {}, r.standards.map((s) => h('tr', {}, h('td', { text: s.framework }), h('td', { text: s.reference }), h('td', { text: s.relevance }))))
            )
          ),
          r.standards_note ? h('p', { class: 'note', text: r.standards_note }) : null
        )
      );
    }

    blocks.push(
      h(
        'div',
        { class: 'block' },
        h('h3', { text: 'Detector signals' }),
        h(
          'div',
          { class: 'table-wrap' },
          h(
            'table',
            {},
            h(
              'thead',
              {},
              h('tr', {}, ['Detector', 'Verdict', 'Confidence', 'Detail', 'ms'].map((t, i) => h('th', { scope: 'col', class: i === 2 || i === 4 ? 'num' : null, text: t })))
            ),
            h(
              'tbody',
              {},
              r.signals.map((s) =>
                h(
                  'tr',
                  {},
                  h('td', { text: detectorName(s.detector) }),
                  h('td', {}, h('span', { class: `verdict ${VERDICT[s.verdict] || ''}`, text: words(s.verdict) })),
                  h('td', { class: 'num', text: s.confidence.toFixed(2) }),
                  h('td', { text: s.detail }),
                  h('td', { class: 'num', text: s.cost_ms })
                )
              )
            )
          )
        )
      )
    );

    blocks.push(
      h(
        'div',
        { class: 'r-actions' },
        h('a', { class: 'btn btn-ghost btn-sm', href: `/api/report/${id}`, target: '_blank', rel: 'noopener' }, 'View JSON report'),
        h('button', { class: 'btn btn-ghost btn-sm', type: 'button', onclick: () => download(`rsashield-report-${id}.json`, r) }, 'Download JSON')
      )
    );
    return h('div', {}, blocks);
  }

  function showError(err) {
    $('#reportEmpty').hidden = true;
    const body = $('#reportBody');
    body.hidden = false;
    const title = err.status === 400 ? 'Could not parse that key' : err.status ? `Request failed (${err.status})` : 'Server unreachable';
    body.replaceChildren(
      h(
        'div',
        { class: 'alert', role: 'alert' },
        h('h2', { text: title }),
        h('p', { text: err.message }),
        err.status ? null : h('p', {}, 'Start it with ', h('code', { text: 'cargo run --release -- serve' }), '.')
      )
    );
    if (!err.status) checkHealth();
  }

  async function openStored(id) {
    try {
      const report = await api('GET', `/api/report/${id}`);
      results = [{ id, report }];
      showResult(0);
      $('#report').scrollIntoView({ behavior: 'smooth', block: 'start' });
    } catch (err) {
      showError(err);
    }
  }

  /* -------------------------------------------------- stored keys + GCD */

  async function refreshKeys() {
    let data;
    try {
      data = await api('GET', '/api/keys');
    } catch {
      return;
    }
    status.querySelector('.status-text').textContent = `Online · ${data.keys.length} stored`;
    const tbody = $('#keysBody');
    if (!data.keys.length) {
      tbody.replaceChildren(h('tr', { class: 'placeholder' }, h('td', { colspan: 6, text: 'No keys stored yet.' })));
      return;
    }
    tbody.replaceChildren(
      ...data.keys.map((k) => {
        const modes = realModes(k.failure_modes);
        const open = () => openStored(k.id);
        return h(
          'tr',
          { tabindex: 0, onclick: open, onkeydown: (e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), open()) },
          h('td', { text: k.id }),
          h('td', { class: 'mono', text: k.key }),
          h('td', { text: k.modulus_bits }),
          h('td', {}, badge(k.classification)),
          h('td', { class: 'num', text: k.risk_score }),
          h('td', {}, modes.length ? h('div', { class: 'chips' }, modes.map((m) => h('span', { class: 'chip', text: words(m) }))) : '—')
        );
      })
    );
  }

  async function runBatch() {
    const out = $('#batchResult');
    $('#batchBtn').disabled = true;
    try {
      const r = await api('POST', '/api/batch-gcd');
      out.className = r.keys_affected ? 'batch-result hit' : 'batch-result';
      const summary = h(
        'p',
        {},
        h('strong', { text: `${r.keys} keys · ${r.pairs} pairs compared · ` }),
        r.keys_affected ? `${r.keys_affected} keys share a prime` : '0 shared factors',
        ` (${r.elapsed_ms} ms, product/remainder tree)`
      );
      out.replaceChildren(summary, r.shared.length ? h('ul', {}, r.shared.map((s) => h('li', { text: `#${s.id} ${s.key} — ${s.detail}` }))) : null);
      out.hidden = false;
      await refreshKeys();
      const current = results.length === 1 && r.shared.find((s) => s.id === results[0].id);
      if (current) openStored(current.id);
    } catch (err) {
      out.className = 'batch-result';
      out.replaceChildren(h('p', { text: err.message }));
      out.hidden = false;
    } finally {
      $('#batchBtn').disabled = !online;
    }
  }

  async function clearKeys() {
    try {
      await api('DELETE', '/api/keys');
    } catch {
      return;
    }
    $('#batchResult').hidden = true;
    results = [];
    $('#reportBody').hidden = true;
    $('#reportEmpty').hidden = false;
    refreshKeys();
  }

  /* ------------------------------------------------------ research bundle */

  const RAMP = ['#eef2ff', '#c9d6fd', '#93acf9', '#5b84f5', '#2c56db'];
  const rampColor = (rate) => RAMP[Math.min(RAMP.length - 1, Math.floor(rate * (RAMP.length - 1) + 1e-9))];

  async function runBundle() {
    const btn = $('#bundleBtn');
    const busy = $('#bundleBusy');
    const timer = $('#bundleTimer');
    const seed = Math.max(0, parseInt($('#seed').value, 10) || 0);
    const quick = $('#quick').checked;
    btn.disabled = true;
    busy.hidden = false;
    const t0 = Date.now();
    const tick = setInterval(() => (timer.textContent = `Running benchmark and ECM study… ${Math.round((Date.now() - t0) / 1000)} s`), 500);
    try {
      const data = await api('POST', '/api/bundle', JSON.stringify({ seed, quick }));
      renderBundle(data);
    } catch (err) {
      $('#bundleOut').hidden = false;
      $('#bundleOut').replaceChildren(h('div', { class: 'alert', role: 'alert' }, h('h2', { text: 'Bundle failed' }), h('p', { text: err.message })));
    } finally {
      clearInterval(tick);
      busy.hidden = true;
      btn.disabled = !online;
    }
  }

  function renderBundle(d) {
    const m = d.metrics;
    const pct = (x) => x.toFixed(3);
    const tiles = [
      ['Precision', pct(m.precision), `${m.true_positives} TP · ${m.false_positives} FP`],
      ['Recall', pct(m.recall), `${m.false_negatives} missed`],
      ['F1', pct(m.f1), `${m.total_keys} keys, ${m.bits || d.bits}-bit`],
      ['False-positive rate', pct(m.false_positive_rate), `${m.healthy} healthy keys`],
      ['“Secure” labels', String(m.secure_labels_emitted), 'by design, never emitted'],
      ['Score separation', `${m.healthy_score_max} | ${m.defective_score_min}`, 'healthy max | defective min'],
    ];

    const tileRow = h(
      'div',
      { class: 'tiles' },
      tiles.map(([label, value, sub]) => h('div', { class: 'tile' }, h('div', { class: 'label', text: label }), h('div', { class: 'value', text: value }), h('div', { class: 'sub', text: sub })))
    );

    const classes = Object.entries(m.per_class_recall);
    const bars = h(
      'div',
      { class: 'panel' },
      h('h3', { text: 'Recall by defect class' }),
      h(
        'ul',
        { class: 'bars' },
        classes.map(([cls, [hit, total]]) => {
          const rate = total ? hit / total : 0;
          return h(
            'li',
            { title: `${CLASSES[cls] || words(cls)}: ${hit} of ${total} detected` },
            h('span', {}, CLASSES[cls] || words(cls)),
            h('span', { class: 'track' }, h('span', { class: 'fill', style: `display:block;width:${rate * 100}%` })),
            h('span', { class: 'val', text: `${hit}/${total}` })
          );
        })
      ),
      h('p', { class: 'note', text: `Tree batch GCD across all ${m.total_keys} keys flagged ${m.batch_gcd_keys_flagged} with a shared prime.` })
    );

    const rows = d.ecm_study.rows;
    const budgets = [...new Map(rows.map((r) => [`${r.b1}/${r.curves}`, r])).values()];
    const sizes = [...new Set(rows.map((r) => r.factor_bits))];
    const heat = h(
      'div',
      { class: 'panel' },
      h('h3', { text: 'ECM detection rate by budget' }),
      h(
        'div',
        { class: 'table-wrap heat' },
        h(
          'table',
          {},
          h('thead', {}, h('tr', {}, h('th', { scope: 'col', text: 'Factor' }), budgets.map((b) => h('th', { scope: 'col', text: `B1=${b.b1} · ${b.curves} curves` })))),
          h(
            'tbody',
            {},
            sizes.map((fb) =>
              h(
                'tr',
                {},
                h('td', { text: `${fb}-bit` }),
                budgets.map((b) => {
                  const r = rows.find((x) => x.factor_bits === fb && x.b1 === b.b1 && x.curves === b.curves);
                  const dark = r.detection_rate >= 0.6;
                  return h(
                    'td',
                    {
                      class: 'cell',
                      style: `background:${rampColor(r.detection_rate)};color:${dark ? '#fff' : 'var(--ink)'}`,
                      title: `${fb}-bit factor, B1=${b.b1}, ${b.curves} curves: ${r.detected}/${r.keys} found, ${r.avg_ms.toFixed(0)} ms per key`,
                    },
                    h('b', { text: r.detection_rate.toFixed(2) }),
                    h('small', { text: `${r.avg_ms.toFixed(0)} ms` })
                  );
                })
              )
            )
          )
        )
      ),
      h('div', { class: 'legend' }, h('span', { text: '0' }), h('span', { class: 'ramp' }), h('span', { text: '1.0 detection rate' }))
    );

    const foot = h(
      'div',
      { class: 'bundle-foot' },
      h('span', { text: `Seed ${d.seed} · ${d.bits}-bit moduli · completed in ${(d.elapsed_ms / 1000).toFixed(1)} s` }),
      h('button', { class: 'btn btn-ghost btn-sm', type: 'button', onclick: () => download(`rsashield-bundle-seed${d.seed}.json`, d) }, 'Download bundle JSON')
    );

    const out = $('#bundleOut');
    out.replaceChildren(tileRow, h('div', { class: 'bundle-grid' }, bars, heat), foot);
    out.hidden = false;
  }

  /* ------------------------------------------------------------- helpers */

  function download(name, data) {
    const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }));
    const a = h('a', { href: url, download: name });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  /* ---------------------------------------------------------------- boot */

  $('#auditBtn').addEventListener('click', runAudit);
  $('#clearInput').addEventListener('click', () => {
    $('#keyInput').value = '';
    $('#keyInput').focus();
  });
  $('#keyInput').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) runAudit();
  });
  $('#batchBtn').addEventListener('click', runBatch);
  $('#clearKeys').addEventListener('click', clearKeys);
  $('#bundleBtn').addEventListener('click', runBundle);

  checkHealth().then((ok) => {
    if (!ok) return;
    loadSamples();
    refreshKeys();
  });
})();
