/*
 * scenes.js — builds every isometric illustration on the landing page into
 * its <svg data-scene="..."> placeholder, using the ISO kit from iso.js.
 */
(function () {
  'use strict';

  const { el, slab, rrect, circle, P, text, icon, frame, SERIF } = ISO;
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const WHITE = { face: '#ffffff', side: 'url(#iso-side-w)', stroke: '#c9d2f2' };
  const NAVY = { face: '#a8b8fb', side: 'url(#iso-side-n)', stroke: '#0b1130' };
  const BLUE = { face: '#4677f6', side: 'url(#iso-side-b)', stroke: '#2a50d8' };
  const GHOST = { face: 'rgba(255,255,255,.55)', side: 'rgba(236,240,252,.6)', stroke: '#d5dcf6' };

  // world point whose projection lands on screen (sx, sy) at ground level
  const W = (sx, sy) => [(sx / ISO.C + sy / ISO.S) / 2, (sy / ISO.S - sx / ISO.C) / 2];

  const wrap = (g, cls, delay) =>
    el('g', { class: cls, style: delay != null ? `animation-delay:${delay}s` : null }, g);

  function tile(g, x, y, z, s, name, st = {}) {
    const h = st.h != null ? st.h : Math.max(6, s * 0.2);
    const t = slab(g, 'top', x, y, z, rrect(-s / 2, -s / 2, s, s, s * 0.24), h, Object.assign({}, WHITE, st));
    if (name) icon(t.on({ opacity: st.iconOpacity }), name, s);
    return t;
  }

  // upright tile facing +x (lower-right); (x, y, z) is the bottom centre
  function standing(g, x, y, z, s, name, st = {}) {
    const t = slab(g, 'right', x, y, z + s, rrect(-s / 2, 0, s, s, s * 0.22), st.t || 7, Object.assign({}, WHITE, st));
    if (name) icon(t.on().appendChild(el('g', { transform: `translate(0 ${s / 2})` })), name, s);
    return t;
  }

  function platform(g, L = 230) {
    return slab(g, 'top', 0, 0, 0, rrect(-L / 2, -L / 2, L, L, 28), 5, {
      face: 'url(#iso-plat)',
      side: '#d6defd',
      stroke: '#8fa4f6',
      sw: 1.3,
    });
  }

  function sparkle(g, x, y, z, r = 9) {
    const [sx, sy] = P(x, y, z);
    const s = el('g', { transform: `translate(${sx} ${sy}) scale(1 .62)` }, g);
    const star = el('g', { class: 'twinkle' }, s);
    el('path', { d: `M0 ${-r}C1 -2 2 -1 ${r} 0C2 1 1 2 0 ${r}C-1 2 -2 1 ${-r} 0C-2 -1 -1 -2 0 ${-r}Z`, fill: '#3d6cf4' }, star);
    el('circle', { cx: r + 4, cy: -r + 2, r: 1.6, fill: '#3d6cf4' }, star);
    el('circle', { cx: -r - 3, cy: r - 1, r: 1.3, fill: '#3d6cf4' }, star);
  }

  // A navy block floating above the ground, with dashed guides to its footprint.
  function hoverBlock(g, x, y, z, s = 50, base = 5, delay = 0) {
    const half = s / 2;
    slab(g, 'top', x, y, base, rrect(-half, -half, s, s, 10), 0, { stroke: '#7d92ea', dash: '3 3' });
    const guides = el('g', { stroke: '#7d92ea', 'stroke-dasharray': '3 3', 'stroke-width': 1 }, g);
    for (const [dx, dy] of [[-half, half], [half, half], [half, -half]]) {
      const [x1, y1] = P(x + dx * 0.9, y + dy * 0.9, base);
      const [x2, y2] = P(x + dx * 0.9, y + dy * 0.9, z);
      el('line', { x1, y1, x2, y2 }, guides);
    }
    sparkle(g, x, y, base);
    const w = wrap(g, 'hover', delay);
    return slab(w, 'top', x, y, z, rrect(-half, -half, s, s, 10), s * 0.5, NAVY);
  }

  const panelRows = (g, rows, x, y, gap, colorA = '#c9d4fb', colorB = '#e3e9ff') => {
    for (let i = 0; i < rows; i++) {
      el('rect', { x, y: y + i * gap, width: 9, height: 6, rx: 2, fill: colorA }, g);
      el('rect', { x: x + 13, y: y + i * gap, width: 30 - i * 5, height: 6, rx: 3, fill: colorB }, g);
    }
  };

  /* ---------------------------------------------------------------- hero */

  function hero(svg) {
    const g = el('g', {}, svg);
    el('ellipse', { cx: 0, cy: -24, rx: 260, ry: 160, fill: 'url(#iso-glow)', class: 'pulse' }, g);

    // the rail the output cards ride along
    slab(g, 'top', 0, -330, 0, rrect(-250, -58, 500, 116, 30), 0, { face: 'url(#iso-rail)' });

    const routes = [
      'M-600 32 C-520 -38 -410 -44 -322 -8 S-170 64 -94 60 L-46 32',
      'M-420 256 L-94 66 L-44 37',
      'M-66 136 L-66 270',
      'M-66 136 L-22 111',
      'M-600 132 C-520 104 -470 170 -396 236',
    ];
    const lines = el('g', { fill: 'none', stroke: '#b4bbd2', 'stroke-width': 1.2, 'stroke-dasharray': '2 5', 'stroke-linecap': 'round' }, g);
    routes.forEach((d) => el('path', { d }, lines));
    if (!reduce) {
      [[0, 9, 0], [0, 9, -4.5], [1, 6.5, -1], [1, 6.5, -4.2], [4, 8, -2]].forEach(([i, dur, begin]) => {
        const c = el('circle', { r: 2.6, fill: '#3d6cf4', opacity: 0 }, g);
        el('animateMotion', { path: routes[i], dur: dur + 's', begin: begin + 's', repeatCount: 'indefinite' }, c);
        el('animate', { attributeName: 'opacity', values: '0;1;1;0', keyTimes: '0;.12;.85;1', dur: dur + 's', begin: begin + 's', repeatCount: 'indefinite' }, c);
      });
    }

    // output cards, far to near
    const labels = ['Factor Recovered', 'Standards Mapped', 'Report Ready'];
    const content = [
      (c) => {
        text(c, 'RISK SCORE', { x: -38, y: -36, size: 6.4, anchor: 'start', fill: '#8a90a8', weight: 600, ls: 0.6 });
        text(c, '100', { x: -39, y: -10, size: 25, anchor: 'start', fill: '#11152e', weight: 400, family: SERIF });
        el('rect', { x: -38, y: 2, width: 64, height: 5, rx: 2.5, fill: '#eef1fb' }, c);
        el('rect', { x: -38, y: 2, width: 58, height: 5, rx: 2.5, fill: '#e2553f' }, c);
        el('rect', { x: -38, y: 14, width: 46, height: 4, rx: 2, fill: '#e8ecfa' }, c);
        el('rect', { x: -38, y: 24, width: 30, height: 4, rx: 2, fill: '#e8ecfa' }, c);
      },
      (c) => {
        ['#3d6cf4', '#e2553f', '#f5b62a', '#22a06b', '#7b5cf0'].forEach((fill, k) =>
          el('rect', { x: -38 + k * 15, y: -40, width: 11, height: 11, rx: 2.5, fill }, c)
        );
        ['IEC 62351-9', 'NERC CIP-010', 'NIST SP 800-57'].forEach((t, k) => {
          el('circle', { cx: -36, cy: -16 + k * 13, r: 2, fill: '#3d6cf4' }, c);
          text(c, t, { x: -31, y: -13.5 + k * 13, size: 7, anchor: 'start', fill: '#474c61' });
        });
      },
      (c) => {
        for (let k = 0; k < 5; k++) {
          el('rect', { x: -38 + k * 15, y: -38 + k * 3, width: 10, height: 10, rx: 2, fill: '#22a06b' }, c);
          el('path', { d: `M${-35.5 + k * 15} ${-33 + k * 3}l2 2 3.5-4`, fill: 'none', stroke: '#fff', 'stroke-width': 1.4 }, c);
        }
        el('rect', { x: -38, y: -6, width: 66, height: 4, rx: 2, fill: '#e8ecfa' }, c);
        el('rect', { x: -38, y: 4, width: 50, height: 4, rx: 2, fill: '#e8ecfa' }, c);
      },
    ];
    for (let i = 2; i >= 0; i--) {
      const y = -150 - 125 * i;
      const z = 6 + 12 * i;
      const w = el('g', { opacity: i === 2 ? 0.5 : i === 1 ? 0.9 : 1 }, wrap(g, 'hover', -i * 1.4));
      const card = slab(w, 'top', 0, y, z, rrect(-48, -60, 96, 120, 10), 4, {
        face: 'rgba(255,255,255,.95)',
        side: '#e6eaf9',
        stroke: '#d1d8f4',
      });
      content[i](card.on());
      const pill = slab(w, 'top', 0, y, z + 4, rrect(-47, 46, 94, 22, 11), 4, BLUE);
      text(pill.on(), labels[i], { y: 60.4, size: 9.4, fill: '#fff' });
    }

    // the hub
    const hub = slab(g, 'top', 0, 0, 0, circle(0, 0, 76), 30, { face: '#e2e8ff', side: 'url(#iso-side-n)', stroke: '#0b1130' });
    const top = hub.on();
    el('circle', { r: 73, fill: '#edf1ff', stroke: '#ffffff', 'stroke-width': 2.5 }, top);
    const ring = (r, w, dash, rot) =>
      el('circle', { r, fill: 'none', stroke: '#3d6cf4', 'stroke-width': w, 'stroke-dasharray': dash, transform: `rotate(${rot})` }, top);
    ring(60, 11, '168 20.5 168 20.5', 20);
    ring(41, 10.5, '232 25.6', 120);
    ring(22, 9, '118 20', 300);
    el('circle', { r: 6, fill: '#3d6cf4' }, top);

    // incoming key material
    const tiles = [
      { p: [-520, 26], s: 34, i: 'net' },
      { p: [-458, 44], s: 46, i: 'cert', stand: true },
      { p: [-372, 36], s: 42, i: 'pem', ghost: true },
      { p: [-272, 52], s: 36, i: 'pem' },
      { p: [-92, 62], s: 38, i: 'key' },
      { p: [-190, 122], s: 40, i: 'chip' },
      { p: [-66, 134], s: 34, i: 'lock' },
      { p: [-282, 172], s: 38, i: 'bolt' },
      { p: [-400, 236], s: 36, i: 'hash' },
      { p: [-66, 240], s: 40, i: 'shield' },
    ].sort((a, b) => a.p[1] - b.p[1]);
    tiles.forEach((t, k) => {
      const at = el('g', { transform: `translate(${t.p[0]} ${t.p[1]})` }, wrap(g, 'bob', -k * 0.7));
      if (t.stand) standing(at, 0, 0, 0, t.s, t.i);
      else tile(at, 0, 0, 0, t.s, t.i, t.ghost ? Object.assign({}, GHOST, { stroke: '#efc3ba', iconOpacity: 0.45 }) : {});
    });

    // phones: frame the hub and its nearest tiles rather than shrinking the whole scene
    const narrow = window.matchMedia('(max-width: 900px)');
    const fit = () => svg.setAttribute('viewBox', narrow.matches ? '-340 -340 700 640' : '-600 -500 1200 810');
    fit();
    narrow.addEventListener('change', fit);
  }

  /* --------------------------------------------------------------- steps */

  function step1(svg) {
    const g = el('g', {}, svg);
    platform(g);

    const ghost = slab(g, 'right', -110, -40, 125, rrect(0, 0, 64, 46, 7), 2, GHOST);
    panelRows(ghost.on(), 3, 8, 9, 11);

    slab(g, 'top', -5, -25, 0, rrect(-28, -55, 56, 110, 14), 150, Object.assign({}, WHITE, { stroke: '#c3cdf3', sw: 1.2 }));

    // terminal on the front-left face
    const scr = el('g', { transform: frame('left', -46, 3, 150).matrix() }, g);
    el('rect', { x: 6, y: 14, width: 70, height: 94, rx: 7, fill: '#121a3c' }, scr);
    text(scr, 'BEGIN PUBLIC KEY', { x: 12, y: 25, size: 5.4, anchor: 'start', fill: '#8fa4f6', weight: 600, ls: 0.3 });
    for (let k = 0; k < 4; k++) {
      const y = 34 + k * 15;
      el('rect', { x: 12, y, width: 7, height: 7, rx: 1.5, fill: 'none', stroke: '#fff', 'stroke-width': 1.1 }, scr);
      if (k < 3) el('path', { d: `M13.5 ${y + 3.5}l1.6 1.8 3.2-3.6`, fill: 'none', stroke: '#fff', 'stroke-width': 1.1 }, scr);
      el('rect', { x: 24, y: y + 1, width: 38 - k * 6, height: 5, rx: 2.5, fill: '#fff', opacity: k < 3 ? 0.85 : 0.35 }, scr);
    }
    el('rect', { x: 44, y: 94, width: 26, height: 8, rx: 4, fill: '#3d6cf4' }, scr);

    // dark slot on the front-right face
    const door = el('g', { transform: frame('right', 50, -11, 150).matrix() }, g);
    el('rect', { x: 4, y: 22, width: 21, height: 106, rx: 7, fill: '#1a2247' }, door);
    el('rect', { x: 8, y: 30, width: 4, height: 90, rx: 2, fill: '#2d3871' }, door);

    // a key tile leaning on the cabinet
    const lean = slab(wrap(g, 'bob', -1), 'left', 30, 22, 51, rrect(0, 0, 46, 46, 10), 8, WHITE);
    icon(lean.on().appendChild(el('g', { transform: 'translate(23 23)' })), 'key', 46);
  }

  function step2(svg) {
    const g = el('g', {}, svg);
    platform(g);
    // back to front, laid out like the reference's step 02
    hoverBlock(g, -90, -60, 95, 50, 5, 0);
    tile(g, -31, -73, 5, 50, 'curve');
    tile(g, -87, 11, 5, 50, 'gcd');
    tile(g, 33, -69, 5, 50, 'frac');
    tile(g, -28, 18, 5, 50, 'sqrt');
    hoverBlock(g, 68, 52, 46, 50, 5, -2);
  }

  function step3(svg) {
    const g = el('g', {}, svg);
    platform(g);

    const chart = slab(g, 'left', -165, -35, 118, rrect(0, 0, 112, 78, 8), 3, {
      face: 'rgba(255,255,255,.92)',
      side: '#e7ebfb',
      stroke: '#cfd7f6',
    }).on();
    el('path', { d: 'M8 64L22 52 34 57 48 36 60 42 72 24 86 31 100 15V72H8z', fill: 'rgba(61,108,244,.1)' }, chart);
    el('path', { d: 'M8 64L22 52 34 57 48 36 60 42 72 24 86 31 100 15', fill: 'none', stroke: '#3d6cf4', 'stroke-width': 2, 'stroke-linejoin': 'round' }, chart);
    el('path', { d: 'M8 72H104', stroke: '#e1e6f8' }, chart);
    text(chart, '100', { x: 92, y: 11, size: 8, fill: '#3d6cf4', weight: 600 });

    const side = slab(g, 'right', 30, -70, 118, rrect(0, 0, 92, 70, 8), 3, {
      face: 'rgba(255,255,255,.72)',
      side: '#eef1fb',
      stroke: '#dbe1f7',
    }).on();
    el('circle', { cx: 26, cy: 34, r: 15, fill: 'none', stroke: '#dfe5fd', 'stroke-width': 7 }, side);
    el('circle', { cx: 26, cy: 34, r: 15, fill: 'none', stroke: '#8ea7f8', 'stroke-width': 7, 'stroke-dasharray': '60 100', transform: 'rotate(-90 26 34)' }, side);
    [18, 30, 22, 38].forEach((h, k) => el('rect', { x: 52 + k * 9, y: 54 - h, width: 6, height: h, rx: 1.5, fill: '#c9d4fb' }, side));

    // the report tower
    slab(g, 'top', -10, -10, 5, rrect(-34, -34, 68, 68, 12), 10, { face: '#1b2350', side: 'url(#iso-side-n)', stroke: '#0b1130' });
    let topSlab;
    for (let k = 0; k < 5; k++) {
      topSlab = slab(g, 'top', -10, -10, 15 + k * 17, rrect(-32, -32, 64, 64, 11), 13, WHITE);
    }
    icon(topSlab.on(), 'target', 40);
    hoverBlock(g, -10, -10, 138, 54, 96, -1);

    const card = slab(wrap(g, 'bob', -2), 'right', 62, 52, 70, rrect(0, 0, 74, 52, 9), 3, WHITE).on();
    el('rect', { x: 8, y: 9, width: 32, height: 20, rx: 8, fill: '#3d6cf4' }, card);
    el('path', { d: 'M17 19l4 4 7-8', fill: 'none', stroke: '#fff', 'stroke-width': 2, 'stroke-linecap': 'round' }, card);
    el('rect', { x: 34, y: 23, width: 32, height: 20, rx: 8, fill: '#11152e' }, card);
    text(card, '!', { x: 50, y: 37.5, size: 12, fill: '#fff', weight: 700 });
  }

  /* ------------------------------------------------------------ examples */

  function panelAt(g, sx, sy, w, h, t, st) {
    const [x, y] = W(sx, sy);
    return slab(g, 'right', x, y, 0, rrect(0, 0, w, h, 7), t, st);
  }

  function exShared(svg) {
    const g = el('g', {}, svg);
    [[-78, -62], [-46, -80]].forEach(([sx, sy]) => panelRows(panelAt(g, sx, sy, 56, 70, 2, GHOST).on(), 4, 8, 12, 12));

    const a = panelAt(wrap(g, 'hover', 0), -118, -6, 92, 64, 3, { face: '#e4e9ff', side: '#c9d4fb', stroke: '#b5c3f7' }).on();
    el('rect', { x: 8, y: 40, width: 50, height: 15, rx: 7.5, fill: '#c9d4fb' }, a);
    text(a, 'Key A', { x: 33, y: 50.6, size: 8, fill: '#3d6cf4', weight: 600 });

    const b = panelAt(wrap(g, 'hover', -1.5), -28, 8, 100, 78, 7, { face: '#ffffff', side: '#10163a', stroke: '#10163a' }).on();
    panelRows(b, 2, 12, 14, 12, '#c9d4fb', '#e8ecfa');
    el('rect', { x: 38, y: 50, width: 54, height: 16, rx: 8, fill: '#3d6cf4' }, b);
    text(b, 'Key B', { x: 65, y: 61, size: 8, fill: '#fff', weight: 600 });

    el('path', { d: 'M-70 62 C-40 92 10 96 34 84', fill: 'none', stroke: '#7d92ea', 'stroke-dasharray': '3 3' }, g);
    const p = el('g', { transform: 'translate(-6 92)' }, wrap(g, 'bob', -0.5));
    const chip = tile(p, 0, 0, 0, 30, null, Object.assign({}, NAVY, { h: 8 }));
    text(chip.on(), 'p', { y: 4.5, size: 13, fill: '#11152e', family: SERIF, style: 'italic' });
  }

  function exClose(svg) {
    const g = el('g', {}, svg);
    el('path', { d: 'M-50 -2 C-30 -28 -20 -36 2 -44', fill: 'none', stroke: '#7d92ea', 'stroke-dasharray': '3 3' }, g);
    el('path', { d: 'M56 -30 C92 -24 108 -8 96 8', fill: 'none', stroke: '#7d92ea', 'stroke-dasharray': '3 3' }, g);

    const tr = el('g', { transform: 'translate(22 -44)' }, wrap(g, 'bob', -1));
    tile(tr, 0, 0, 0, 62, 'sqrt');

    const tl = el('g', { transform: 'translate(-70 22)' }, wrap(g, 'bob', 0));
    tile(tl, 0, 0, 0, 62, 'key');
    el('path', { d: 'M-70 -10V-30', stroke: '#e2553f', 'stroke-width': 1.2 }, g);
    el('circle', { cx: -70, cy: -40, r: 10, fill: '#e2553f' }, g);
    text(g, 'p≈q', { x: -70, y: -37, size: 7.4, fill: '#fff', weight: 700 });

    const br = el('g', { transform: 'translate(92 22)' }, wrap(g, 'bob', -2));
    const n = tile(br, 0, 0, 0, 48, null, Object.assign({}, NAVY, { h: 10 }));
    const sp = n.on();
    el('path', { d: 'M0 -9C1 -2 2 -1 9 0C2 1 1 2 0 9C-1 2 -2 1 -9 0C-2 -1 -1 -2 0 -9Z', fill: '#3d6cf4' }, sp);
  }

  function exECM(svg) {
    const g = el('g', {}, svg);
    const back = panelAt(g, -18, -88, 70, 66, 2, GHOST).on();
    [14, 24, 18, 32].forEach((h, k) => el('rect', { x: 10 + k * 12, y: 54 - h, width: 7, height: h, rx: 1.5, fill: '#dfe5fd' }, back));

    const donut = panelAt(g, -112, -36, 58, 58, 0, { face: 'none' }).on();
    el('circle', { cx: 29, cy: 29, r: 18, fill: 'none', stroke: '#e3e9ff', 'stroke-width': 9 }, donut);
    el('circle', { cx: 29, cy: 29, r: 18, fill: 'none', stroke: '#9fb3f9', 'stroke-width': 9, 'stroke-dasharray': '40 120' }, donut);

    const c = panelAt(g, -62, -56, 122, 92, 3, Object.assign({}, WHITE, { stroke: '#c3cdf3' })).on();
    text(c, 'factor found', { x: 112, y: 16, size: 7.5, anchor: 'end', fill: '#3d6cf4', weight: 600 });
    el('path', { d: 'M8 84H114M12 88V10', stroke: '#e1e6f8' }, c);
    el('path', { d: 'M14 74C34 70 36 42 52 38C66 35 72 48 64 54C56 60 62 76 104 82', fill: 'none', stroke: '#3d6cf4', 'stroke-width': 2, 'stroke-linecap': 'round' }, c);
    el('circle', { cx: 52, cy: 38, r: 3.6, fill: '#e2553f' }, c);

    const stack = el('g', { transform: 'translate(-78 60)' }, g);
    tile(stack, 0, 0, 0, 52, null, { h: 11 });
    tile(stack, 0, 0, 14, 52, null, { h: 11 });
    const top = el('g', {}, wrap(stack, 'hover', -1));
    tile(top, 0, 0, 34, 52, null, Object.assign({}, NAVY, { h: 22 }));
  }

  function exWiener(svg) {
    const g = el('g', {}, svg);
    const lines = el('g', { stroke: '#c8cee2', 'stroke-dasharray': '3 4', fill: 'none', 'data-bleed': '' }, g);
    const line = (a, b) => {
      const [x1, y1] = P(...a);
      const [x2, y2] = P(...b);
      el('line', { x1, y1, x2, y2 }, lines);
    };
    line([-60, 260], [-60, -260]);
    line([60, 260], [60, -260]);
    line([-260, 0], [260, 0]);

    // (u, v) cells; world x = v, y = -u
    const cells = [
      [-90, -90, 'w'], [30, -90, 'w'], [-30, -30, 'w'], [90, -30, 'w'], [-90, 30, 'w'],
      [30, 30, 'n'], [-30, 90, 'w'], [90, 90, 'w'], [-90, 90, 'b'],
    ].sort((a, b) => a[1] - a[0] - (b[1] - b[0]));
    const pins = [];
    for (const [u, v, kind] of cells) {
      const st = kind === 'n' ? Object.assign({}, NAVY, { h: 10 }) : kind === 'b' ? Object.assign({}, BLUE, { h: 7 }) : { h: 6, stroke: '#d0d7f3' };
      tile(g, v, -u, 0, 44, null, st);
      if (kind !== 'w' || (u === 30 && v === -90)) pins.push([P(v, -u, 0), kind]);
    }
    for (const [[sx, sy], kind] of pins) {
      const pin = el('g', { transform: `translate(${sx} ${sy - 30})` }, wrap(g, 'bob', -sx / 40));
      el('path', { d: 'M0 14C-5 7-7 3-7 0a7 7 0 0 1 14 0c0 3-2 7-7 14z', fill: kind === 'n' ? '#11152e' : '#3d6cf4' }, pin);
      el('circle', { r: 2.6, fill: '#fff' }, pin);
    }
    text(g, 'k/d', { x: P(-30, -30)[0] + 26, y: P(-30, -30)[1] - 26, size: 10, style: 'italic', family: SERIF, fill: '#8a90a8' });
  }

  function exMeta(svg) {
    const g = el('g', {}, svg);
    const [x0, y0] = W(30, 45);

    const base = slab(g, 'top', x0, y0, 0, rrect(-70, -40, 140, 80, 10), 8, {
      face: '#eef1ff',
      side: 'url(#iso-side-l)',
      stroke: '#8ea3f5',
      sw: 1.2,
    }).on();
    el('rect', { x: -60, y: -32, width: 120, height: 34, rx: 5, fill: 'none', stroke: '#c9d4fb' }, base);
    el('rect', { x: -20, y: 9, width: 40, height: 24, rx: 4, fill: '#dfe6ff', stroke: '#8ea3f5', 'stroke-width': 0.8 }, base);

    const scr = slab(g, 'right', x0 - 40, y0 + 70, 98, rrect(0, 0, 140, 90, 9), 6, {
      face: '#ffffff',
      side: 'url(#iso-side-l)',
      stroke: '#8ea3f5',
      sw: 1.2,
    }).on();
    el('rect', { x: 8, y: 8, width: 124, height: 74, rx: 6, fill: '#f7f8ff', stroke: '#dfe5fd' }, scr);
    text(scr, '512-bit', { x: 70, y: 48, size: 22, fill: '#11152e', weight: 400, family: SERIF });
    text(scr, 'below 2048-bit policy', { x: 70, y: 64, size: 7.6, fill: '#8a90a8' });
    icon(scr.appendChild(el('g', { transform: 'translate(70 22)' })), 'warn', 18);

    el('path', { d: 'M-128 6 C-110 30 -90 34 -70 30', fill: 'none', stroke: '#7d92ea', 'stroke-dasharray': '3 3' }, g);
    const card = panelAt(wrap(g, 'hover', 0), -168, -32, 46, 56, 4, WHITE).on();
    el('circle', { cx: 23, cy: 20, r: 9, fill: 'none', stroke: '#3d6cf4', 'stroke-width': 2 }, card);
    text(card, 'e = 1', { x: 23, y: 43, size: 8, fill: '#11152e', weight: 600 });
    text(card, 'e', { x: 23, y: 23.5, size: 10, fill: '#3d6cf4', family: SERIF, style: 'italic' });

    const chat = el('g', { transform: 'translate(112 70)' }, wrap(g, 'bob', -1.2));
    standing(chat, 0, 0, 0, 40, 'shield');
  }

  /* ------------------------------------------------------------ features */

  function featHonest(svg) {
    const g = el('g', {}, svg);
    const [x, y] = W(-70, 70);
    slab(g, 'top', x, y, 0, rrect(-170, -170, 340, 340, 30), 0, { face: 'url(#iso-blue)' }).g.setAttribute('data-bleed', '');
    slab(g, 'top', x, y, 0, rrect(-120, -120, 240, 240, 24), 0, { stroke: 'rgba(255,255,255,.5)', dash: '4 4' }).g.setAttribute('data-bleed', '');
    const [px, py] = W(60, 34);
    slab(g, 'top', px, py, 0, circle(0, 0, 40), 14, WHITE);
    slab(g, 'top', px, py, 14, circle(0, 0, 30), 6, { face: '#eef1ff', side: '#dfe5fd', stroke: '#c9d2f2' });
    const tall = el('g', { transform: `translate(60 34)` }, wrap(g, 'hover', 0));
    standing(tall, 0, 0, 30, 54, 'shield');
    const tag = el('g', { transform: 'translate(132 -42)' }, g);
    el('rect', { x: -48, y: -10, width: 96, height: 20, rx: 10, fill: '#fff', stroke: '#c9d2f2' }, tag);
    text(tag, 'NO_DEFECT_FOUND', { y: 3.4, size: 7.6, fill: '#3d6cf4', weight: 700, ls: 0.3 });
  }

  function featICS(svg) {
    const g = el('g', {}, svg);
    const grid = el('g', { stroke: '#d6dbee', 'stroke-dasharray': '3 3', fill: 'none', 'data-bleed': '' }, g);
    for (let k = -8; k <= 8; k++) {
      const a = P(k * 34, -400);
      const b = P(k * 34, 400);
      el('line', { x1: a[0], y1: a[1], x2: b[0], y2: b[1] }, grid);
      const c = P(-400, k * 34);
      const d = P(400, k * 34);
      el('line', { x1: c[0], y1: c[1], x2: d[0], y2: d[1] }, grid);
    }
    const labelled = [
      [W(-70, -10), 'IEC', '62351'],
      [W(10, -46), 'NERC', 'CIP-010'],
      [W(84, -6), 'NIST', '800-57'],
    ];
    labelled.forEach(([[x, y], a, b], k) => {
      const t = tile(wrap(g, 'bob', -k), x, y, 0, 58, null, { h: 12 }).on();
      text(t, a, { y: -1, size: 12, fill: '#11152e', weight: 700 });
      text(t, b, { y: 10, size: 7.5, fill: '#3d6cf4', weight: 600 });
    });
    const [bx, by] = W(8, 32);
    tile(wrap(g, 'bob', -2.4), bx, by, 0, 58, 'bolt', { h: 12 });
  }

  function featMeasured(svg) {
    const g = el('g', {}, svg);
    const [x, y] = W(-6, -30);
    slab(g, 'right', x, y, 0, rrect(0, 0, 260, 200, 10), 0, { face: 'url(#iso-blue)' }).g.setAttribute('data-bleed', '');
    const panel = el('g', { transform: frame('right', x, y, 0).matrix() }, g);
    [['Precision', '1.000'], ['Recall', '1.000'], ['F1', '1.000']].forEach(([k, v], i) => {
      const yy = 22 + i * 26;
      el('rect', { x: 22, y: yy, width: 118, height: 18, rx: 4, fill: 'none', stroke: 'rgba(255,255,255,.6)' }, panel);
      text(panel, k, { x: 30, y: yy + 12.4, size: 8, anchor: 'start', fill: '#fff', weight: 500 });
      text(panel, v, { x: 132, y: yy + 12.4, size: 8, anchor: 'end', fill: '#fff', weight: 700 });
    });
    const badge = el('g', { transform: 'translate(-104 6)' }, wrap(g, 'hover', 0));
    el('circle', { r: 30, fill: 'url(#iso-badge)', stroke: '#b9c6f6', 'stroke-width': 1.4 }, badge);
    icon(badge, 'target', 36);
    const tag = el('g', { transform: 'translate(150 -30)' }, wrap(g, 'bob', -1));
    standing(tag, 0, 0, 0, 46, 'hash');
  }

  /* ----------------------------------------------------------------- cta */

  function orbit(svg) {
    const g = el('g', {}, svg);
    const ringC = [340, 230];
    [92, 176, 260].forEach((r, k) => {
      el('circle', { cx: ringC[0], cy: ringC[1], r, fill: 'none', stroke: '#eef1ff', 'stroke-width': 40 }, g);
      el('circle', { cx: ringC[0], cy: ringC[1], r: r - 20, fill: 'none', stroke: '#d3dcfc', 'stroke-width': 1 }, g);
      el('circle', { cx: ringC[0], cy: ringC[1], r: r + 20, fill: 'none', stroke: '#d3dcfc', 'stroke-width': 1, 'stroke-dasharray': k === 1 ? '420 60' : null }, g);
    });
    const arcC = [40, 230];
    const R = 236;
    const dots = el('g', { fill: 'none', stroke: '#9aa6c9', 'stroke-dasharray': '2 5' }, g);
    el('circle', { cx: arcC[0], cy: arcC[1], r: R + 54, opacity: 0.8 }, dots);
    el('circle', { cx: arcC[0], cy: arcC[1], r: R - 54, opacity: 0.8 }, dots);

    const names = ['pem', 'key', 'cert', 'chip', 'shield', 'bolt', 'lock', 'net'];
    const badges = names.map((name) => {
      const b = el('g', {}, g);
      el('ellipse', { cx: 4, cy: 10, rx: 46, ry: 44, fill: '#c9d4fb', opacity: 0.45 }, b);
      el('circle', { r: 48, fill: 'url(#iso-badge)', stroke: '#b7c3f5', 'stroke-width': 1.5 }, b);
      el('circle', { r: 41, fill: 'none', stroke: '#e3e9fd', 'stroke-width': 1 }, b);
      icon(b, name, 62);
      return b;
    });
    const spacing = 27;
    const place = (t) =>
      badges.forEach((b, i) => {
        const deg = ((((i * spacing + t) % (spacing * names.length)) + spacing * names.length) % (spacing * names.length)) - 110;
        const a = (deg * Math.PI) / 180;
        b.setAttribute('transform', `translate(${(arcC[0] + R * Math.cos(a)).toFixed(1)} ${(arcC[1] + R * Math.sin(a)).toFixed(1)})`);
        b.setAttribute('opacity', Math.abs(deg) > 100 ? 0 : 1);
      });
    place(0);
    if (reduce) return;
    let visible = false;
    new IntersectionObserver(([e]) => (visible = e.isIntersecting)).observe(svg);
    let t = 0;
    let last = performance.now();
    const tick = (now) => {
      if (visible) {
        t += ((now - last) / 1000) * 5;
        place(t);
      }
      last = now;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  /* ------------------------------------------------------------------ boot */

  const SCENES = {
    hero,
    step1,
    step2,
    step3,
    'ex-shared': exShared,
    'ex-close': exClose,
    'ex-ecm': exECM,
    'ex-wiener': exWiener,
    'ex-meta': exMeta,
    'feat-honest': featHonest,
    'feat-ics': featICS,
    'feat-measured': featMeasured,
    orbit,
  };

  /*
   * Re-frame a scene around what it actually drew, keeping the authored aspect
   * ratio so cards in a row stay the same height. Elements marked data-bleed
   * (backdrops, grids, guide lines) are meant to run off the edge and are left
   * out of the measurement. The padding leaves room for the bob/hover motion.
   */
  function fit(svg) {
    const [, , vw, vh] = svg.getAttribute('viewBox').split(/[\s,]+/).map(Number);
    const bleed = [...svg.querySelectorAll('[data-bleed]')];
    bleed.forEach((n) => n.setAttribute('display', 'none'));
    const b = svg.firstElementChild.getBBox();
    bleed.forEach((n) => n.removeAttribute('display'));
    if (!b.width || !b.height) return;
    const pad = 0.07 * Math.max(b.width, b.height) + 10;
    const aspect = vw / vh;
    const w = Math.max(b.width + 2 * pad, (b.height + 2 * pad) * aspect);
    const h = w / aspect;
    const x = b.x + b.width / 2 - w / 2;
    const y = b.y + b.height / 2 - h / 2;
    svg.setAttribute('viewBox', [x, y, w, h].map((v) => v.toFixed(1)).join(' '));
  }

  ISO.defs();
  document.querySelectorAll('svg[data-scene]').forEach((svg) => {
    const build = SCENES[svg.dataset.scene];
    if (!build) return;
    build(svg);
    if (!('fixed' in svg.dataset)) fit(svg);
  });
})();
