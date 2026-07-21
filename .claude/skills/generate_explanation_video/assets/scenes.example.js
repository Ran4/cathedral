/* scenes.example.js — a minimal 3-scene template.
 *
 * Each scene id here MUST match an id in narration.example.json. A scene is
 * `defScene(id, build)` where build(root) runs once and returns { update(lt) }.
 * `lt` is local time in seconds since this scene's narration started; every
 * visual is a pure function of lt (no timers, no rAF) so any frame can be
 * rendered in isolation and deterministically.
 *
 * Helpers available from engine.js:
 *   A(lt, start, dur, ease?)  -> 0..1 progress of a sub-animation
 *   window_(lt, start, end)   -> fade in / hold / fade out
 *   pulse(lt, start, dur)     -> 0 -> 1 -> 0
 *   el / svg / at / rise      -> dom + layout helpers
 *   glyph(kind, scale, opts)  -> inline SVG icon (define your own kinds)
 *   rr(x,y,w,h,r)             -> rounded-rect path
 *   lerp, fmt, clamp01, EASE
 * X0/Y0/CW are the content-box origin and width (defined below).
 */

const X0 = 92, Y0 = 132, CW = 1736;

/* ---- 1. title -------------------------------------------------------- */
defScene('s01_title', root => {
  const wrap = el('div', null, at(X0, 300, { width: CW + 'px' }));
  root.appendChild(wrap);
  const kicker = el('div', 'mono',
    { fontSize: '17px', letterSpacing: '.34em', color: 'var(--gold-dim)', marginBottom: '28px' },
    'YOUR PROJECT · SUBSYSTEM');
  const h = el('div', 'h1', { fontSize: '96px', lineHeight: '1.03' }, 'Example&nbsp;Title');
  const sub = el('div', null,
    { fontSize: '28px', color: 'var(--text-2)', marginTop: '24px', fontFamily: '"DejaVu Serif", serif' },
    'A one-line subtitle describing the piece');
  wrap.append(kicker, h, sub);
  return {
    update(lt) {
      rise(kicker, A(lt, 0.2, 1.0), 14);
      rise(h, A(lt, 0.6, 1.1), 28);
      rise(sub, A(lt, 1.4, 1.0), 18);
    },
  };
});

/* ---- 2. a left-to-right flow ---------------------------------------- */
defScene('s02_flow', root => {
  const head = el('div', 'h2', at(X0, Y0, {}), 'A pipeline in three steps.');
  root.appendChild(head);

  const NODES = [
    { t: 'Input', s: 'where it starts' },
    { t: 'Transform', s: 'the work happens' },
    { t: 'Output', s: 'the result' },
  ];
  const NW = 380, GAP = 130, NY = 300;
  const xOf = i => 40 + i * (NW + GAP);
  const inner = `
    <defs><marker id="ah" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
      <path d="M0,0 L9,4.5 L0,9 z" fill="#6b5f4a"/></marker></defs>
    ${NODES.map((n, i) => `<g id="n${i}" opacity="0">
      <path d="${rr(xOf(i), NY, NW, 120, 5)}" fill="#201c17" stroke="#3d352a"/>
      <text x="${xOf(i) + 22}" y="${NY + 46}" font-family="DejaVu Serif" font-size="26"
            fill="#efe6d3">${n.t}</text>
      <text x="${xOf(i) + 22}" y="${NY + 84}" font-family="DejaVu Sans" font-size="17"
            fill="#ab9f89">${n.s}</text></g>`).join('')}
    ${[0, 1].map(i => `<path id="e${i}" d="M${xOf(i) + NW},${NY + 60} L${xOf(i) + NW},${NY + 60}"
        stroke="#6b5f4a" stroke-width="1.6" marker-end="url(#ah)"/>`).join('')}
    <g id="tok" opacity="0"></g>`;
  const holder = svg(CW, 460, inner, at(X0, Y0 + 60));
  root.appendChild(holder);
  const S = holder.querySelector('svg');
  const beats = [0.8, 3.4, 6.0];

  return {
    update(lt) {
      rise(head, A(lt, 0.1, 0.8), 16);
      NODES.forEach((n, i) => {
        const p = A(lt, beats[i], 0.8);
        const g = S.querySelector('#n' + i);
        g.setAttribute('opacity', fmt(p, 2));
        g.setAttribute('transform', `translate(0,${fmt((1 - p) * 16, 1)})`);
      });
      [0, 1].forEach(i => {
        const p = A(lt, beats[i + 1] - 0.5, 0.55);
        const e = S.querySelector('#e' + i);
        e.setAttribute('opacity', p > 0 ? '1' : '0');   // never draw a zero-length arrow
        e.setAttribute('d', `M${xOf(i) + NW},${NY + 60} L${lerp(xOf(i) + NW, xOf(i + 1) - 10, p)},${NY + 60}`);
      });
      // a token travelling node 0 -> node 2
      const tok = S.querySelector('#tok');
      if (lt > 6.5 && lt < 9.5) {
        const p = A(lt, 6.6, 2.6, 'inOut');
        const x = lerp(xOf(0) + NW - 30, xOf(2) + 30, p);
        tok.setAttribute('opacity', fmt(window_(lt, 6.6, 9.2, 0.3), 2));
        tok.innerHTML = `<circle cx="${x}" cy="${NY + 60}" r="16" fill="#1c1915" stroke="#d9a441"/>`;
      } else tok.setAttribute('opacity', '0');
    },
  };
});

/* ---- 3. a code / data-shape scene ----------------------------------- */
defScene('s03_code', root => {
  const head = el('div', 'h2', at(X0, Y0, {}), 'A data shape.');
  root.appendChild(head);

  const code = el('div', 'code', at(X0, Y0 + 100, { width: '900px', fontSize: '24px' }));
  code.innerHTML =
    `<span class="kw">struct</span> <span class="ty">Example</span> {\n` +
    `    id:    <span class="ty">String</span>,\n` +
    `    <span class="add">weight: f32,</span>          <span class="cm">// the field that matters</span>\n` +
    `    kind:  <span class="ty">Kind</span>,\n}`;
  root.appendChild(code);

  const note = el('div', null, at(X0, Y0 + 420, { width: '1000px' }));
  note.innerHTML = `<div style="font-size:24px;line-height:1.5;color:var(--text-2)">
    Talk over the code and point at the one thing that matters — here,
    <span class="mono" style="color:var(--gold)">weight</span>. Use
    <span class="mono">.del</span> / <span class="mono">.add</span> spans to show a before/after diff.</div>`;
  root.appendChild(note);

  return {
    update(lt) {
      rise(head, A(lt, 0.1, 0.8), 16);
      rise(code, A(lt, 0.9, 0.9), 18);
      rise(note, A(lt, 3.4, 0.9), 18);
    },
  };
});
