/*
 * iso.js — a tiny isometric SVG kit for the RSAShield illustrations.
 *
 * World axes: x runs screen right-down, y runs screen left-down, z runs up.
 * Every shape is a flat outline drawn on one of three planes ("top", "left",
 * "right") and then extruded along that plane's normal, which is enough for
 * the tiles, slabs, disks and standing panels used on the landing page.
 */
(function (global) {
  'use strict';

  const NS = 'http://www.w3.org/2000/svg';
  const C = Math.cos(Math.PI / 6);
  const S = 0.5;

  // world -> screen
  const P = (x, y, z = 0) => [(x - y) * C, (x + y) * S - z];

  // Where a plane's local u (right) and v (down) axes land on screen, and the
  // screen direction of its outward normal (used for extrusion).
  const PLANES = {
    top: { u: [C, -S], v: [C, S], n: [0, -1] }, // text runs up-right
    left: { u: [C, S], v: [0, 1], n: [-C, S] }, // face toward +y
    right: { u: [C, -S], v: [0, 1], n: [C, S] }, // face toward +x
  };

  function el(tag, attrs, parent) {
    const node = document.createElementNS(NS, tag);
    if (attrs) for (const k in attrs) if (attrs[k] != null) node.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(node);
    return node;
  }

  function frame(plane, x, y, z) {
    const b = PLANES[plane];
    const [ox, oy] = P(x, y, z);
    return {
      map: (u, v, lift = 0) => [
        ox + b.u[0] * u + b.v[0] * v + b.n[0] * lift,
        oy + b.u[1] * u + b.v[1] * v + b.n[1] * lift,
      ],
      matrix: (lift = 0) =>
        `matrix(${b.u[0]} ${b.u[1]} ${b.v[0]} ${b.v[1]} ${ox + b.n[0] * lift} ${oy + b.n[1] * lift})`,
    };
  }

  // Rounded rectangle outline in local plane coordinates.
  function rrect(x, y, w, h, r = 0, seg = 5) {
    r = Math.max(0, Math.min(r, w / 2, h / 2));
    if (!r) return [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    const pts = [];
    const corners = [
      [x + w - r, y + r, -Math.PI / 2],
      [x + w - r, y + h - r, 0],
      [x + r, y + h - r, Math.PI / 2],
      [x + r, y + r, Math.PI],
    ];
    for (const [cx, cy, a0] of corners) {
      for (let i = 0; i <= seg; i++) {
        const a = a0 + (i / seg) * (Math.PI / 2);
        pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
      }
    }
    return pts;
  }

  function circle(cx, cy, r, n = 56) {
    const pts = [];
    for (let i = 0; i < n; i++) {
      const a = (i / n) * Math.PI * 2;
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
    return pts;
  }

  // Convex hull (monotone chain); the silhouette of an extruded convex shape.
  function hull(points) {
    const p = points.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    const lower = [];
    const upper = [];
    for (const q of p) {
      while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], q) <= 0) lower.pop();
      lower.push(q);
    }
    for (let i = p.length - 1; i >= 0; i--) {
      const q = p[i];
      while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], q) <= 0) upper.pop();
      upper.push(q);
    }
    upper.pop();
    lower.pop();
    return lower.concat(upper);
  }

  const path = (pts) => 'M' + pts.map((p) => p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join('L') + 'Z';

  /*
   * Extrude a local outline on a plane by `t` along the plane normal.
   * Style: face, side, stroke, sw, dash. Returns { g, on } where on() opens a
   * group whose coordinates are the front face's local (u, v) plane.
   */
  function slab(parent, plane, x, y, z, outline, t, st = {}) {
    const f = frame(plane, x, y, z);
    const g = el('g', { class: st.cls, opacity: st.opacity }, parent);
    const common = {
      stroke: st.stroke || 'none',
      'stroke-width': st.sw || 1,
      'stroke-linejoin': 'round',
      'stroke-dasharray': st.dash,
    };
    const front = outline.map(([u, v]) => f.map(u, v, t));
    if (t > 0) {
      const back = outline.map(([u, v]) => f.map(u, v, 0));
      el('path', Object.assign({ d: path(hull(back.concat(front))), fill: st.side || '#e5e9f6' }, common), g);
    }
    el('path', Object.assign({ d: path(front), fill: st.face || 'none' }, common), g);
    return { g, on: (attrs) => el('g', Object.assign({ transform: f.matrix(t) }, attrs || {}), g) };
  }

  function text(parent, str, o = {}) {
    const node = el(
      'text',
      {
        x: o.x || 0,
        y: o.y || 0,
        'font-size': o.size || 10,
        'font-weight': o.weight || 500,
        'font-style': o.style,
        fill: o.fill || '#11152e',
        'text-anchor': o.anchor || 'middle',
        'letter-spacing': o.ls,
        'font-family': o.family || 'Inter, system-ui, sans-serif',
        opacity: o.opacity,
      },
      parent
    );
    node.textContent = str;
    return node;
  }

  const SERIF = 'Lora, Georgia, serif';

  // Flat glyphs drawn in a 40-unit box centred on the origin.
  const ICONS = {
    pem(g) {
      el('path', { d: 'M-9 -13h11l7 7v20h-18z', fill: '#fff', stroke: '#e2553f', 'stroke-width': 1.6, 'stroke-linejoin': 'round' }, g);
      el('path', { d: 'M2 -13v7h7', fill: 'none', stroke: '#e2553f', 'stroke-width': 1.6, 'stroke-linejoin': 'round' }, g);
      el('rect', { x: -13, y: 0, width: 19, height: 9, rx: 2, fill: '#e2553f' }, g);
      text(g, 'PEM', { x: -3.5, y: 6.8, size: 6.2, weight: 700, fill: '#fff' });
    },
    cert(g) {
      el('rect', { x: -12, y: -11, width: 24, height: 18, rx: 2.5, fill: '#fff', stroke: '#3d6cf4', 'stroke-width': 1.6 }, g);
      el('path', { d: 'M-8 -5h12M-8 -1h9', stroke: '#9fb3f9', 'stroke-width': 1.6, 'stroke-linecap': 'round' }, g);
      el('path', { d: 'M3 7l-2 7 4-2 4 2-2-7', fill: '#e2553f' }, g);
      el('circle', { cx: 6, cy: 5, r: 4.6, fill: '#f5b62a', stroke: '#d4900d', 'stroke-width': 1 }, g);
    },
    key(g) {
      el('circle', { cx: -6, cy: 0, r: 6, fill: 'none', stroke: '#f0a52b', 'stroke-width': 3.6 }, g);
      el('path', { d: 'M0 0h14M9 0v5M13 0v4', stroke: '#f0a52b', 'stroke-width': 3.6, 'stroke-linecap': 'round', fill: 'none' }, g);
    },
    chip(g) {
      const pins = el('g', { stroke: '#8c98c9', 'stroke-width': 1.8, 'stroke-linecap': 'round' }, g);
      for (const k of [-5, 0, 5]) {
        el('path', { d: `M${k} -13v3M${k} 10v3M-13 ${k}h3M10 ${k}h3` }, pins);
      }
      el('rect', { x: -10, y: -10, width: 20, height: 20, rx: 3.5, fill: '#1b2350' }, g);
      el('rect', { x: -5, y: -5, width: 10, height: 10, rx: 2, fill: '#3d6cf4' }, g);
    },
    bolt(g) {
      el('path', { d: 'M3 -14L-8 2h8l-3 12L10 -3H2z', fill: '#f5b62a', stroke: '#d4900d', 'stroke-width': 1.2, 'stroke-linejoin': 'round' }, g);
    },
    lock(g) {
      el('path', { d: 'M-6 -2v-4a6 6 0 0 1 12 0v4', fill: 'none', stroke: '#3d6cf4', 'stroke-width': 3 }, g);
      el('rect', { x: -9.5, y: -3, width: 19, height: 15, rx: 3, fill: '#3d6cf4' }, g);
      el('circle', { cx: 0, cy: 4, r: 2.2, fill: '#fff' }, g);
    },
    net(g) {
      el('path', { d: 'M-9 7L0 -8L9 7z', fill: 'none', stroke: '#22a06b', 'stroke-width': 2 }, g);
      for (const [cx, cy] of [[-9, 7], [9, 7], [0, -8]]) el('circle', { cx, cy, r: 4.6, fill: '#22a06b' }, g);
    },
    shield(g) {
      el('path', { d: 'M0 -13l11 4.5v8c0 7-5 11.5-11 14-6-2.5-11-7-11-14v-8z', fill: '#3d6cf4' }, g);
      el('path', { d: 'M-5 0.5l3.5 3.5 6.5-7', fill: 'none', stroke: '#fff', 'stroke-width': 2.4, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, g);
    },
    hash(g) {
      text(g, '#', { y: 7.5, size: 22, weight: 700, fill: '#7b5cf0' });
    },
    target(g) {
      el('circle', { r: 12, fill: 'none', stroke: '#3d6cf4', 'stroke-width': 3 }, g);
      el('circle', { r: 6.5, fill: 'none', stroke: '#3d6cf4', 'stroke-width': 3 }, g);
      el('circle', { r: 2.2, fill: '#3d6cf4' }, g);
    },
    warn(g) {
      el('path', { d: 'M0 -12L12 10H-12z', fill: '#f5b62a', stroke: '#d4900d', 'stroke-width': 1.2, 'stroke-linejoin': 'round' }, g);
      el('path', { d: 'M0 -3v6', stroke: '#11152e', 'stroke-width': 2.2, 'stroke-linecap': 'round' }, g);
      el('circle', { cx: 0, cy: 6.6, r: 1.3, fill: '#11152e' }, g);
    },
    gcd(g) {
      text(g, 'gcd', { y: 4, size: 13, style: 'italic', weight: 500, fill: '#1b2350', family: SERIF });
    },
    sqrt(g) {
      text(g, '√n', { y: 5, size: 15, weight: 500, fill: '#3d6cf4', family: SERIF });
    },
    curve(g) {
      el('path', { d: 'M-12 11C-3 7-6-4 2-7c6-2 10 2 6 6-3 3-1 7 4 10', fill: 'none', stroke: '#e2553f', 'stroke-width': 2.4, 'stroke-linecap': 'round' }, g);
      el('circle', { cx: 2, cy: -7, r: 2.6, fill: '#3d6cf4' }, g);
    },
    frac(g) {
      text(g, 'k/d', { y: 5, size: 13, style: 'italic', weight: 500, fill: '#22a06b', family: SERIF });
    },
  };

  function icon(parent, name, size = 40) {
    const g = el('g', { transform: `scale(${size / 40})` }, parent);
    ICONS[name](g);
    return g;
  }

  // Gradients and filters shared by every illustration on the page.
  function defs() {
    if (document.getElementById('iso-defs')) return;
    const svg = el('svg', {
      id: 'iso-defs',
      width: 0,
      height: 0,
      'aria-hidden': 'true',
      style: 'position:absolute;width:0;height:0;overflow:hidden',
    });
    const d = el('defs', {}, svg);
    const stops = (g, list) =>
      list.forEach(([offset, color, a = 1]) => el('stop', { offset, 'stop-color': color, 'stop-opacity': a }, g));
    const lin = (id, list, a) => stops(el('linearGradient', Object.assign({ id, x1: 0, y1: 0, x2: 1, y2: 0 }, a), d), list);
    const rad = (id, list, a) => stops(el('radialGradient', Object.assign({ id }, a), d), list);

    lin('iso-side-w', [[0, '#f8f9fe'], [1, '#d8def2']]);
    lin('iso-side-n', [[0, '#2b3468'], [1, '#080c22']]);
    lin('iso-side-l', [[0, '#e3e9ff'], [1, '#bcc9fa']]);
    lin('iso-side-b', [[0, '#3a63e6'], [1, '#1f3fae']]);
    lin('iso-rail', [[0, '#d2dbfd', 0.9], [1, '#d2dbfd', 0]]);
    lin('iso-blue', [[0, '#4a77f7'], [1, '#2f5ae6']], { x2: 0.6, y2: 1 });
    rad('iso-glow', [[0, '#4f7bff', 0.42], [0.45, '#88a5ff', 0.16], [1, '#c9d5ff', 0]]);
    rad('iso-plat', [[0, '#fdfdff'], [0.6, '#edf1ff'], [1, '#dbe3ff']]);
    rad('iso-badge', [[0, '#ffffff'], [0.75, '#f5f7ff'], [1, '#e2e8ff']], { cx: 0.45, cy: 0.4 });
    const blur = el('filter', { id: 'iso-blur', x: '-50%', y: '-50%', width: '200%', height: '200%' }, d);
    el('feGaussianBlur', { stdDeviation: 2.4 }, blur);
    const soft = el('filter', { id: 'iso-soft', x: '-30%', y: '-30%', width: '160%', height: '160%' }, d);
    el('feDropShadow', { dx: 0, dy: 6, stdDeviation: 7, 'flood-color': '#3d6cf4', 'flood-opacity': 0.14 }, soft);
    document.body.prepend(svg);
  }

  global.ISO = { C, S, P, el, frame, rrect, circle, hull, slab, text, icon, defs, SERIF };
})(window);
