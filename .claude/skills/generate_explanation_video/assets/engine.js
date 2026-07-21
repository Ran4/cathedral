/* engine.js — deterministic time-driven animation framework.
   Everything is a pure function of the global timeline position `t`, so the
   renderer can screenshot any frame in any order and get identical output. */

/* ---------------------------------------------------------------- easing */
const clamp01 = x => x < 0 ? 0 : x > 1 ? 1 : x;
const EASE = {
  linear: x => x,
  out:    x => 1 - Math.pow(1 - x, 3),
  outQ:   x => 1 - Math.pow(1 - x, 4),
  inOut:  x => x < .5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2,
  back:   x => { const c = 1.70158, c3 = c + 1; return 1 + c3 * Math.pow(x - 1, 3) + c * Math.pow(x - 1, 2); },
  pop:    x => x < .5 ? 2 * x * x : 1 - Math.pow(-2 * x + 2, 2) / 2,
};

/** Progress of a sub-animation starting at `start`, lasting `dur`. */
function A(lt, start, dur, ease = 'out') {
  return EASE[ease](clamp01((lt - start) / dur));
}
/** 0 -> 1 -> 0 pulse, peaking mid-window. */
function pulse(lt, start, dur) {
  const p = clamp01((lt - start) / dur);
  return Math.sin(p * Math.PI);
}
/** Fade in at `start`, hold, fade out at `end`. */
function window_(lt, start, end, fade = .45) {
  return Math.min(A(lt, start, fade), 1 - A(lt, end, fade));
}
const lerp = (a, b, x) => a + (b - a) * x;
const fmt = (x, n = 0) => x.toFixed(n);

/* ------------------------------------------------------------------- dom */
function el(tag, cls, css, html) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (css) Object.assign(n.style, css);
  if (html != null) n.innerHTML = html;
  return n;
}
function svg(w, h, inner, css) {
  const s = el('div', null, css);
  s.innerHTML = `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${inner}</svg>`;
  return s;
}
/** Absolute-position helper inside a scene's content box. */
function at(x, y, extra) {
  return Object.assign({ position: 'absolute', left: x + 'px', top: y + 'px' }, extra || {});
}
/** Reveal helper: opacity + slight rise. */
function rise(node, p, dy = 22) {
  node.style.opacity = p;
  node.style.transform = `translateY(${(1 - p) * dy}px)`;
}

/* ----------------------------------------------------------------- icons */
const C = {
  grain: '#cfa52a', flour: '#e6dcc2', loaf: '#b8763a',
  wool: '#bdb6a5', kersey: '#6e8395', broad: '#8465ac', spark: '#f2c451',
};

/** Commodity glyphs, drawn at a nominal 40x40 box and scaled by `s`. */
function glyph(kind, s = 1, opts = {}) {
  const o = opts.o != null ? opts.o : 1;
  const g = {
    grain: `<path d="M12 14 q8-6 16 0 l4 20 q-12 5 -24 0 z" fill="${C.grain}" opacity=".92"/>
            <path d="M12 14 q8-6 16 0 l-2 3 q-6-3 -12 0 z" fill="#8a6d18"/>
            <path d="M15 22 h10 M14 27 h12" stroke="#8a6d18" stroke-width="1.4" opacity=".5"/>`,
    flour: `<path d="M12 14 q8-6 16 0 l4 20 q-12 5 -24 0 z" fill="${C.flour}" opacity=".95"/>
            <path d="M12 14 q8-6 16 0 l-2 3 q-6-3 -12 0 z" fill="#a99c7d"/>
            <path d="M20 20 v13" stroke="#a99c7d" stroke-width="1.4" opacity=".55"/>`,
    loaf:  `<ellipse cx="20" cy="25" rx="13" ry="9" fill="${C.loaf}"/>
            <ellipse cx="20" cy="23.5" rx="13" ry="9" fill="#d08f4e"/>
            <path d="M13 21 l4-4 M19 20.5 l4-4 M25 21 l3.5-3.5" stroke="#8d5526" stroke-width="1.6" stroke-linecap="round"/>`,
    wool:  `<rect x="8" y="15" width="24" height="18" rx="4" fill="${C.wool}"/>
            <path d="M8 21 h24 M8 27 h24" stroke="#7d776a" stroke-width="1.6"/>
            <path d="M14 15 v18 M26 15 v18" stroke="#7d776a" stroke-width="1.6"/>`,
    kersey:`<rect x="9" y="16" width="22" height="16" rx="2" fill="${C.kersey}"/>
            <path d="M9 20 h22 M9 28 h22" stroke="#4d5f6e" stroke-width="1.3"/>
            <ellipse cx="9" cy="24" rx="2.5" ry="8" fill="#5b6f80"/>`,
    broad: `<rect x="9" y="16" width="22" height="16" rx="2" fill="${C.broad}"/>
            <path d="M9 20 h22 M9 28 h22" stroke="#5f4a83" stroke-width="1.3"/>
            <ellipse cx="9" cy="24" rx="2.5" ry="8" fill="#6f579b"/>`,
    spark: `<circle cx="20" cy="24" r="9" fill="${C.spark}"/>
            <circle cx="20" cy="24" r="9" fill="none" stroke="#a8811f" stroke-width="1.4"/>
            <path d="M20 19 l1.6 3.4 3.4 .6 -2.5 2.4 .6 3.6 -3.1-1.8 -3.1 1.8 .6-3.6 -2.5-2.4 3.4-.6z" fill="#7d5f14"/>`,
    person:`<circle cx="20" cy="14" r="5.5" fill="currentColor"/>
            <path d="M9.5 34 q0-11 10.5-11 t10.5 11 z" fill="currentColor"/>`,
  }[kind] || '';
  return `<g transform="translate(${opts.x || 0},${opts.y || 0}) scale(${s})" opacity="${o}">${g}</g>`;
}

/** A small stack-of-N badge: glyph + xN count. */
function stackSvg(kind, n, s = 1) {
  return `${glyph(kind, s)}<text x="${34 * s}" y="${32 * s}" font-family="DejaVu Sans Mono"
          font-size="${15 * s}" fill="#ab9f89">x${n}</text>`;
}

/* --------------------------------------------------------- shared pieces */

/** The seven canonical offices of the day. */
const OFFICES = [
  { k: 'watch',     n: 'Watch',     h: '02' },
  { k: 'kindling',  n: 'Kindling',  h: '05' },
  { k: 'dayspring', n: 'Dayspring', h: '07' },
  { k: 'high_wick', n: 'High Wick', h: '12' },
  { k: 'waning',    n: 'Waning',    h: '15' },
  { k: 'lamplight', n: 'Lamplight', h: '18' },
  { k: 'snuffing',  n: 'Snuffing',  h: '21' },
];
const WEEKDAYS = ['Bellday', 'Second', 'Highmarket', 'Fourth', 'Fifth', 'Lowmarket', 'Seventh'];

/**
 * Horizontal office strip. Returns {node, set(activeKeys, litKeys)} where
 * `activeKeys` glow gold and `litKeys` get a dim highlight.
 */
function officeStrip(w) {
  const cellW = w / OFFICES.length;
  const inner = OFFICES.map((o, i) => {
    const x = i * cellW;
    return `<g data-k="${o.k}">
      <rect class="bgr" x="${x + 3}" y="0" width="${cellW - 6}" height="58" rx="3"
            fill="#221e18" stroke="#3d352a"/>
      <text class="lbl" x="${x + cellW / 2}" y="24" text-anchor="middle"
            font-family="DejaVu Sans" font-size="17" fill="#ab9f89">${o.n}</text>
      <text class="hr" x="${x + cellW / 2}" y="44" text-anchor="middle"
            font-family="DejaVu Sans Mono" font-size="13" fill="#7d735f">${o.h}:00</text>
    </g>`;
  }).join('');
  const node = svg(w, 58, inner);
  const S = node.querySelector('svg');
  return {
    node,
    set(active = [], lit = []) {
      OFFICES.forEach(o => {
        const g = S.querySelector(`g[data-k="${o.k}"]`);
        const on = active.includes(o.k), dim = lit.includes(o.k);
        g.querySelector('.bgr').setAttribute('fill', on ? '#3a2f18' : dim ? '#2a251d' : '#221e18');
        g.querySelector('.bgr').setAttribute('stroke', on ? '#d9a441' : dim ? '#55493a' : '#3d352a');
        g.querySelector('.lbl').setAttribute('fill', on ? '#f2c451' : dim ? '#efe6d3' : '#ab9f89');
      });
    },
  };
}

/** Rounded rect path helper. */
function rr(x, y, w, h, r) {
  return `M${x + r},${y} h${w - 2 * r} a${r},${r} 0 0 1 ${r},${r} v${h - 2 * r}
          a${r},${r} 0 0 1 -${r},${r} h-${w - 2 * r} a${r},${r} 0 0 1 -${r},-${r}
          v-${h - 2 * r} a${r},${r} 0 0 1 ${r},-${r} z`;
}

/** An arrow between two points with an animated draw-on. */
function arrowPath(x1, y1, x2, y2) {
  return `M${x1},${y1} L${x2},${y2}`;
}

/* -------------------------------------------------------------- registry */
const SCENES = {};
function defScene(id, build) { SCENES[id] = build; }

let _built = null;
const $scenes = () => document.getElementById('scenes');

function buildAll() {
  if (_built) return _built;
  _built = {};
  for (const s of TIMINGS.scenes) {
    const holder = el('div', 'scene');
    $scenes().appendChild(holder);
    const api = SCENES[s.id] ? SCENES[s.id](holder) : { update() {} };
    _built[s.id] = { holder, api, meta: s };
  }
  return _built;
}

/* --------------------------------------------------------------- render */
const XFADE = 0.5;   // seconds of crossfade at a scene boundary

function renderAt(t) {
  const built = buildAll();
  const scenes = TIMINGS.scenes;

  // Which scene owns this instant? A scene owns from its start until the next
  // scene's start; the gap between them belongs to the outgoing scene.
  let idx = 0;
  for (let i = 0; i < scenes.length; i++) {
    if (t >= scenes[i].start - (i === 0 ? TIMINGS.lead_in : XFADE)) idx = i;
  }
  const cur = scenes[idx];
  const nxt = scenes[idx + 1];

  // opacity of the current scene, and of the incoming one during a crossfade
  let curOp = 1, nxtOp = 0;
  if (idx === 0) curOp = clamp01((t - (cur.start - TIMINGS.lead_in)) / 0.9);
  if (nxt) {
    const xs = nxt.start - XFADE;
    if (t > xs) { nxtOp = clamp01((t - xs) / XFADE); curOp = 1 - nxtOp; }
  } else {
    // tail fade-out at the very end
    curOp = Math.min(curOp, 1 - clamp01((t - (cur.end + 1.4)) / 1.1));
  }

  for (const s of scenes) {
    const b = built[s.id];
    const active = s.id === cur.id || (nxt && s.id === nxt.id && nxtOp > 0);
    b.holder.classList.toggle('on', !!active);
    if (!active) continue;
    const op = s.id === cur.id ? curOp : nxtOp;
    b.holder.style.opacity = op;
    b.api.update(t - s.start, s);
  }

  // chrome
  const shown = (nxt && nxtOp > .5) ? nxt : cur;
  const n = scenes.indexOf(shown) + 1;
  document.getElementById('sc-num').textContent =
    String(n).padStart(2, '0') + ' / ' + String(scenes.length).padStart(2, '0');
  document.getElementById('sc-title').textContent = shown.title;
  document.getElementById('prog-fill').style.width = (100 * clamp01(t / TIMINGS.total)) + '%';

  // the title card hides the chrome
  const chromeOp = clamp01((t - (scenes[0].end - 1.0)) / 1.0);
  document.getElementById('chrome-top').style.opacity = chromeOp;
  document.getElementById('chrome-rule').style.opacity = chromeOp;
  document.getElementById('chrome-foot').style.opacity = chromeOp;
}
window.renderAt = renderAt;
