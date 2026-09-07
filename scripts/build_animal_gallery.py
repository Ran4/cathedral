#!/usr/bin/env python3
"""Build the offline animal notebook from gauntlet/animal_gallery.json.

Usage: UV_CACHE_DIR=/tmp/animal-uv-cache uv run scripts/build_animal_gallery.py
       uv run scripts/build_animal_gallery.py --check

No dependencies. By default every image and video is embedded once, so the HTML
can be copied and viewed offline. --linked instead uses relative media paths.
Source-register links remain relative to the repository in either mode.

Manifest version 1:
  animals[]: id, name, subtitle, description, status, focus[], stages[],
    comparison: {before: stage id, after: stage id, default_view, views[]},
    stages[]: id, title, label, status, capture: repository-relative directory,
      optional image: explicit repository-relative lead image,
      description, changes[], optional images: {view: repository-relative path}.
    motions[]: title, status, description, video (optional), poster (optional),
      frames_dir, frame_indices[], fps; or frames: [{image, label}], note.
    limitations[]. A comparison with matching stage ids is a baseline viewer.
  performance: baseline/current JSON paths (current nullable), budgets_ms, note.
  process[]: title, description. sources[]: title, path, description.
  title, edition, status, introduction, capture_note, limitations[].

Paths resolve from repository root. Missing captures are errors, never silently
replaced. Only game captures and gauntlet/gallery assets may be embedded.
"""

from __future__ import annotations

import argparse
import base64
import html
import json
import mimetypes
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
VIEW_NAMES = {"three_quarter": "Three-quarter", "side": "Side", "front": "Front", "rear": "Rear", "street_distance": "Street distance"}


def esc(value: object) -> str:
    return html.escape(str(value), quote=True)


def safe_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")


def local_file(value: str) -> Path:
    path = (ROOT / value).resolve()
    if not path.is_relative_to(ROOT) or not path.is_file():
        raise ValueError(f"Missing or non-repository file: {value}")
    return path


class Gallery:
    def __init__(self, manifest: dict, output: Path, linked: bool):
        self.data = manifest
        self.output = output
        self.linked = linked
        self.media: dict[str, str] = {}
        self.paths: dict[Path, str] = {}
        self.comparisons = {}

    def asset(self, value: str) -> str:
        path = local_file(value)
        capture_manifest = path.parent / "capture_manifest.json"
        if capture_manifest.is_file() and json.loads(capture_manifest.read_text()).get("completed") is False:
            raise ValueError(f"Capture run did not complete: {path.parent}")
        allowed = (ROOT / "captures/animals", ROOT / "gauntlet/gallery")
        if not any(path.is_relative_to(root) for root in allowed):
            raise ValueError(f"Gallery media must be game captures, not reference footage: {value}")
        if path in self.paths:
            return self.paths[path]
        key = f"m{len(self.paths)}"
        mime = mimetypes.guess_type(path)[0]
        if not mime or not mime.startswith(("image/", "video/")):
            raise ValueError(f"Unsupported media: {value}")
        self.paths[path] = key
        self.media[key] = (os.path.relpath(path, self.output.parent) if self.linked else
                           f"data:{mime};base64," + base64.b64encode(path.read_bytes()).decode("ascii"))
        return key

    def picture(self, path: str, alt: str, *, zoom: bool = True, css: str = "") -> str:
        key = self.asset(path)
        img = f'<img data-media="{key}" alt="{esc(alt)}" loading="lazy" decoding="async" width="1120" height="840">'
        if not zoom:
            return img
        return f'<button class="image-button {esc(css)}" type="button" data-zoom="{key}" data-caption="{esc(alt)}" aria-label="Enlarge: {esc(alt)}">{img}<span class="zoom-cue" aria-hidden="true">↗</span></button>'

    @staticmethod
    def stage_image(stage: dict, view: str = "three_quarter") -> str:
        return stage.get("images", {}).get(view, f'{stage["capture"]}/{view}.png')

    def lead_image(self, animal: dict) -> str:
        stage = next(s for s in animal["stages"] if s["id"] == animal["comparison"]["after"])
        return stage.get("image") or self.stage_image(stage)

    @staticmethod
    def lead_label(animal: dict) -> str:
        return next(s["label"] for s in animal["stages"] if s["id"] == animal["comparison"]["after"])

    def comparison(self, animal: dict) -> str:
        aid = animal["id"]
        config = animal["comparison"]
        stages = {stage["id"]: stage for stage in animal["stages"]}
        before, after = stages[config["before"]], stages[config["after"]]
        paired = before["id"] != after["id"]
        views = config.get("views", list(VIEW_NAMES))
        default = config.get("default_view", views[0])
        if default not in views:
            raise ValueError(f"Unknown default view for {aid}: {default}")
        records = {}
        tabs = []
        for view in views:
            records[view] = {"before": self.asset(self.stage_image(before, view)), "after": self.asset(self.stage_image(after, view)), "label": VIEW_NAMES.get(view, view)}
            tabs.append(f'<button type="button" class="view-tab" data-view="{esc(view)}" aria-pressed="{str(view == default).lower()}">{esc(VIEW_NAMES.get(view, view))}</button>')
        self.comparisons[aid] = records
        b, a = records[default]["before"], records[default]["after"]
        range_control = f'''<div class="compare-control"><label for="range-{aid}">Reveal the change <span>Drag, or use arrow keys</span></label><input id="range-{aid}" type="range" min="0" max="100" value="50" aria-label="Amount of original {esc(animal['name'])} shown" aria-valuetext="50 percent original, 50 percent revised"><div class="range-ends"><span>{esc(before['label'])}</span><span>{esc(after['label'])}</span></div></div>''' if paired else '<p class="baseline-note">Original capture · a comparison will appear when a revised stage is recorded.</p>'
        overlay = f'<div class="compare-before"><img data-media="{b}" alt="Original {esc(animal["name"])} — {esc(VIEW_NAMES.get(default, default))}" width="1120" height="840"></div><div class="compare-line" aria-hidden="true"><span>↔</span></div><span class="image-label original-label">Original</span>' if paired else ""
        return f'''<div class="comparison {'is-paired' if paired else 'is-baseline'}" data-comparison="{aid}" data-current-view="{esc(default)}">
          <div class="viewer-toolbar"><span class="eyebrow">{'Then / now' if paired else 'The starting point'}</span><div class="view-tabs" role="group" aria-label="{esc(animal['name'])} camera view">{''.join(tabs)}</div></div>
          <div class="compare-stage" style="--split:50%"><img class="compare-after" data-media="{a}" alt="{esc(after['title'])} — {esc(VIEW_NAMES.get(default, default))}" width="1120" height="840">{overlay}<span class="image-label current-label">{esc(after['status'])}</span></div>
          {range_control}<div class="viewer-footer"><span class="view-status" aria-live="polite">{esc(VIEW_NAMES.get(default, default))} · matched studio camera</span><div><button type="button" class="text-button open-before" data-zoom="{b}" data-caption="{esc(before['title'])}">Enlarge original ↗</button>{f'<button type="button" class="text-button open-after" data-zoom="{a}" data-caption="{esc(after["title"])}">Enlarge revision ↗</button>' if paired else ''}</div></div>
        </div>'''

    def stage(self, stage: dict, animal: dict) -> str:
        picture = self.picture(stage.get("image") or self.stage_image(stage), f'{animal["name"]} · {stage["title"]}')
        changes = ''.join(f'<li>{esc(change)}</li>' for change in stage.get("changes", []))
        return f'''<article class="stage-card"><div class="stage-top"><span class="eyebrow">{esc(stage['label'])}</span><span class="small-status">{esc(stage['status'])}</span></div>{picture}<div class="stage-copy"><h4>{esc(stage['title'])}</h4><p>{esc(stage['description'])}</p>{f'<ul>{changes}</ul>' if changes else ''}</div></article>'''

    def motion(self, motion: dict) -> str:
        frames = motion.get("frames")
        if frames is None:
            fps = motion.get("fps", 12)
            if fps <= 0:
                raise ValueError("Motion fps must be positive")
            frames = []
            for index in motion.get("frame_indices", []):
                path = f'{motion["frames_dir"]}/frame_{index:03}.png'
                record = (ROOT / path).with_suffix('.json')
                time = json.loads(record.read_text()).get('time_seconds') if record.is_file() else None
                label = f't = {time:.2f} s' if time is not None else f'{index/fps:.2f} s into clip'
                frames.append({"image": path, "label": label})
        poster = motion.get("poster") or (frames[0]["image"] if frames else None)
        poster_attr = f' data-poster="{self.asset(poster)}"' if poster else ''
        if motion.get("video"):
            video = f'<video controls muted loop playsinline preload="none" data-media="{self.asset(motion["video"])}"{poster_attr} aria-label="{esc(motion["title"])}"></video>'
        else:
            video = self.picture(poster, motion["title"]) if poster else ''
        strip = ''.join(f'<figure>{self.picture(frame["image"], motion["title"] + " · " + frame["label"])}<figcaption>{esc(frame["label"])}</figcaption></figure>' for frame in frames)
        return f'''<article class="motion-card"><div class="motion-heading"><h4>{esc(motion['title'])}</h4><span class="small-status">{esc(motion.get('status', 'Recorded'))}</span></div><p>{esc(motion['description'])}</p>{video}<div class="filmstrip" style="grid-template-columns:repeat({min(6, max(1, len(frames)))},minmax(0,1fr))" aria-label="Captured motion samples">{strip}</div><p class="motion-note">{esc(motion.get('note', 'Samples run from left to right in time.'))}</p></article>'''

    def animal(self, animal: dict, index: int) -> str:
        stages = ''.join(self.stage(stage, animal) for stage in animal["stages"])
        motion = ''.join(self.motion(m) for m in animal.get("motions", []))
        focus = ''.join(f'<span>{esc(item)}</span>' for item in animal.get("focus", []))
        limitations = ''.join(f'<li>{esc(item)}</li>' for item in animal.get("limitations", []))
        return f'''<section class="animal-section" id="{animal['id']}"><header class="section-heading"><div class="section-number">0{index}</div><div><div class="heading-line"><p class="eyebrow">{esc(animal['subtitle'])}</p><span class="status-pill">{esc(animal['status'])}</span></div><h2>{esc(animal['name'])}</h2><p class="section-description">{esc(animal['description'])}</p><div class="focus-list" aria-label="Anatomy under review">{focus}</div></div></header>
          {self.comparison(animal)}
          <div class="subheading"><h3>The shape of the work</h3><span>{len(animal['stages']):02} recorded {'stage' if len(animal['stages']) == 1 else 'stages'} · tap an image to inspect</span></div><div class="stage-grid">{stages}</div>
          {f'<div class="subheading"><h3>Between the stills</h3><span>Silent clips · inspectable frame samples</span></div><div class="motion-grid">{motion}</div>' if motion else ''}
          {f'<aside class="animal-caveat"><strong>Still to resolve</strong><ul>{limitations}</ul></aside>' if limitations else ''}</section>'''

    def performance(self) -> str:
        config = self.data.get("performance")
        if not config:
            return ''
        baseline = json.loads(local_file(config["baseline"]).read_text())
        current = json.loads(local_file(config["current"]).read_text()) if config.get("current") else None
        rows = []
        for key, label in (("dogs", "Dog animation"), ("rats", "Rat pose and geometry")):
            old = baseline[key]
            new = current[key] if current else None
            budget = config.get("budgets_ms", {}).get(key)
            num = lambda value: f'{value:.4f}'
            rows.append(f'<tr><th scope="row">{label}<span>{old["count"]} animals · {old["samples"]:,} baseline samples</span></th><td>{num(old["p50_ms"])}<small>p95 {num(old["p95_ms"])}</small></td><td>{num(new["p50_ms"]) if new else "—"}<small>{"p95 " + num(new["p95_ms"]) if new else "Not recorded"}</small></td><td>{f"&lt; {budget:g}" if budget else "—"}</td></tr>')
        counts = []
        allocation_notes = []
        for label, data in (("Baseline", baseline), ("Current", current)):
            if not data:
                continue
            for key in ("dogs", "rats"):
                entry = data[key]
                triangles = entry.get("triangles_drawn", entry.get("triangles"))
                if triangles is not None:
                    counts.append(f'<span>{label} · {entry["count"]} {key}: <strong>{triangles:,}</strong> triangles</span>')
                allocations = entry.get("system_allocations_total", entry.get("geometry_allocations_total"))
                if allocations is not None:
                    allocation_notes.append(f'{label.lower()} {key}: {allocations:,} allocations across {entry["samples"]:,} measured updates')
        allocations_html = '<p>Steady-state allocation counter: ' + '; '.join(allocation_notes) + '.</p>' if allocation_notes else ''
        skin = current.get("skin_cpu") if current else None
        if skin:
            allocations_html += f'<p>Additional skin CPU work: {skin["p50_ms"]:.4f} ms median, {skin["p95_ms"]:.4f} ms p95, across {skin["meshes"]} meshes and {skin["joint_matrices"]} joint matrices. {esc(skin["method"])}.</p>'
        if current and "vertices" in current["rats"]:
            rat_bytes = current["rats"]["vertices"] * 48 + current["rats"]["triangles"] * 12
            original_bytes = baseline["rats"]["vertices"] * 48 + baseline["rats"]["triangles"] * 12
            allocations_html += f'<p>Rat batch attribute/index payload per rebuild: {rat_bytes / 1024:.1f} KiB current, {original_bytes / 1024:.1f} KiB baseline. Calculated from position, normal, UV, color and 32-bit indices; this is a data-size estimate, not measured GPU transfer time, and excludes driver overhead.</p>'
        return f'''<section class="performance-section" id="cost"><div><p class="eyebrow">A small place in the frame budget</p><h2>What it costs.</h2><p>{esc(config['note'])}</p><span class="cpu-label">CPU only · milliseconds</span></div><div class="performance-data"><div class="table-scroll"><table><caption class="sr-only">Animal CPU timings in milliseconds; current measurements may be unavailable.</caption><thead><tr><th scope="col">Workload</th><th scope="col">Baseline median</th><th scope="col">Current median</th><th scope="col">Median budget</th></tr></thead><tbody>{''.join(rows)}</tbody></table></div><div class="geometry-counts">{''.join(counts)}</div><details><summary>Measurement method</summary><p>{esc(baseline['method'])}</p>{f'<p>Current: {esc(current["method"])}</p>' if current else ''}{allocations_html}<p>These timings do not measure full-city frame time or hardware GPU rendering. Geometry totals describe the listed animal counts. CPU timings can vary with other work on this machine; the comparison establishes a budget check, not a claimed speedup.</p></details></div></section>'''

    def build(self) -> str:
        data = self.data
        if data.get("version") != 1:
            raise ValueError("Expected gallery manifest version 1")
        ids = [a["id"] for a in data["animals"]]
        if len(ids) != len(set(ids)) or any(not re.fullmatch(r"[a-z][a-z0-9-]*", i) for i in ids):
            raise ValueError("Animal ids must be unique lowercase HTML identifiers")
        for animal in data["animals"]:
            stage_ids = [stage["id"] for stage in animal["stages"]]
            if len(stage_ids) != len(set(stage_ids)):
                raise ValueError(f'Duplicate stage ids in {animal["id"]}')
        nav = ''.join(f'<a href="#{a["id"]}">{esc(a["name"])}</a>' for a in data['animals'])
        hero = ''.join(f'<figure class="hero-card">{self.picture(self.lead_image(a), a["name"] + " · current recorded stage")}<figcaption><span>{esc(a["name"])}</span><span>{esc(self.lead_label(a))}</span></figcaption></figure>' for a in data['animals'])
        animals = ''.join(self.animal(a, index + 1) for index, a in enumerate(data['animals']))
        process = ''.join(f'<article><span class="eyebrow">0{index + 1}</span><h3>{esc(p["title"])}</h3><p>{esc(p["description"])}</p></article>' for index, p in enumerate(data.get('process', [])))
        limitations = ''.join(f'<li>{esc(item)}</li>' for item in data.get('limitations', []))
        sources = ''.join(f'<a class="source-link" href="{esc(os.path.relpath(local_file(source["path"]), self.output.parent))}"><strong>{esc(source["title"])} ↗</strong><span>{esc(source["description"])}</span></a>' for source in data.get('sources', []))
        performance = self.performance()
        return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="color-scheme" content="light"><meta name="description" content="A local visual notebook of the Cathedral-City’s procedural dogs and rats."><title>{esc(data['title'])} · Cathedral animal notebook</title><style>{CSS}</style></head><body>
<a href="#main" class="skip-link">Skip to the notebook</a>
<header class="site-header"><a class="wordmark" href="#top"><span class="wordmark-mark" aria-hidden="true">✳</span> CATHEDRAL <span class="wordmark-sub">/ Field notes</span></a><nav aria-label="Notebook sections">{nav}<a href="#cost">Cost & evidence</a></nav></header>
<main id="main"><section class="hero" id="top"><div class="hero-copy"><div class="edition"><span>{esc(data['edition'])}</span><span class="live-status">{esc(data['status'])}</span></div><h1>{esc(data['title'])}<span class="title-period">.</span></h1><p class="hero-intro">{esc(data['introduction'])}</p><a class="start-link" href="#{ids[0]}">Open the notebook <span aria-hidden="true">↘</span></a></div><div class="hero-collage">{hero}<span class="margin-note">From silhouette<br>to street life.</span></div></section>
<div class="capture-note"><span class="eyebrow">Inside the workshop</span><p>{esc(data.get('capture_note', ''))}</p><span>Images enlarge ↗</span></div>
<noscript><p class="noscript">Enable JavaScript to view the embedded media and use the comparison controls. This notebook makes no network requests.</p></noscript>
{animals}{performance}
<section class="evidence-section" id="evidence"><div class="subheading"><h2>How to read this notebook</h2><span>The evidence behind the images</span></div><div class="process-grid">{process}</div><div class="evidence-bottom"><div><h3>What the captures can tell us</h3><ul>{limitations}</ul></div><div><h3>Observation & attribution</h3><p>The images and players above show our game captures. External animal reference photos and footage are not embedded. The local source registers preserve their creators, links, and observation limits.</p>{sources}</div></div></section>
</main><footer><span>CATHEDRAL / Animal workshop</span><span>{esc(data['status'])} · {'Self-contained media' if not self.linked else 'Linked local media'} · No network required</span><a href="#top">Back to the top ↑</a></footer>
<dialog id="lightbox" aria-label="Enlarged capture"><div class="lightbox-bar"><p id="lightbox-caption"></p><button type="button" id="lightbox-close" autofocus aria-label="Close enlarged capture">Close <span aria-hidden="true">×</span></button></div><div class="lightbox-image"><img id="lightbox-img" alt=""></div><p class="lightbox-help">Click the image for full resolution · Escape to close</p></dialog>
<script type="application/json" id="media-data">{safe_json(self.media)}</script><script type="application/json" id="comparison-data">{safe_json(self.comparisons)}</script><script>{JS}</script></body></html>'''


CSS = r'''
:root{--paper:#f3eee4;--card:#faf7ef;--ink:#28382f;--muted:#64695e;--line:#cecbbb;--accent:#a14f36;--olive:#4f624b;--dark:#233329;--radius:4px;font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:var(--ink);background:var(--paper);font-synthesis:none}
*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:92px}body{margin:0}button,input{font:inherit}button,a,input{-webkit-tap-highlight-color:transparent}a{color:inherit;text-underline-offset:4px}button{color:inherit;cursor:pointer}button:focus-visible,a:focus-visible,input:focus-visible,summary:focus-visible{outline:3px solid var(--accent);outline-offset:4px}button{border:0}img,video{display:block;width:100%;height:auto}img{color:#fff}h1,h2,h3,h4,p,figure{margin:0}h1,h2,h3,h4{font-family:Georgia,"Times New Roman",serif;font-weight:400}p{line-height:1.65}ul{padding-left:1.15rem}li{padding-left:.2rem;line-height:1.6}li+li{margin-top:.4rem}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.skip-link{position:fixed;top:6px;left:10px;z-index:100;background:var(--card);padding:12px;transform:translateY(-160%)}.skip-link:focus{transform:translateY(0)}
.site-header{height:76px;display:flex;align-items:center;justify-content:space-between;padding:0 max(28px,calc((100vw - 1320px)/2));border-bottom:1px solid var(--line);position:sticky;top:0;background:#f3eee4f5;z-index:20;backdrop-filter:blur(12px);gap:20px}.wordmark{font-size:13px;font-weight:750;letter-spacing:.12em;text-decoration:none;white-space:nowrap;display:flex;align-items:center;gap:9px}.wordmark-mark{font-size:31px;color:var(--accent);line-height:1}.wordmark-sub{font-family:Georgia,serif;letter-spacing:0;font-size:15px;font-style:italic;font-weight:400}.site-header nav{display:flex;gap:28px;font-size:12px;font-weight:650}.site-header nav a{text-decoration:none}.site-header nav a:hover{color:var(--accent)}main{max-width:1320px;margin:0 auto;padding:0 28px}.hero{display:grid;grid-template-columns:1fr 1fr;gap:36px;align-items:center;min-height:640px;padding:66px 0 68px}.edition{display:flex;gap:20px;align-items:center;font-size:10px;text-transform:uppercase;letter-spacing:.1em;font-weight:650;margin-bottom:25px}.live-status{color:var(--accent);white-space:nowrap}.live-status:before{content:"";display:inline-block;width:6px;height:6px;border-radius:100%;background:var(--accent);margin-right:6px}.hero h1{font-size:clamp(54px,6.6vw,92px);line-height:.99;letter-spacing:-.055em;max-width:570px;text-wrap:balance}.title-period{color:var(--accent)}.hero-intro{font-size:15px;max-width:460px;color:var(--muted);margin-top:28px}.start-link{display:inline-flex;gap:45px;margin-top:30px;padding-bottom:9px;border-bottom:1px solid var(--ink);text-decoration:none;font-size:12px;font-weight:650}.start-link span{font-size:18px}.hero-collage{position:relative;min-height:540px;padding:8px 22px 45px}.hero-card{background:var(--card);padding:10px;box-shadow:0 10px 35px #28382f18;border:1px solid #d9d5c8;width:83%;transform:rotate(-4deg);position:relative;z-index:1}.hero-card .image-button{aspect-ratio:4/3}.hero-card figcaption{display:flex;justify-content:space-between;font-size:9px;color:var(--muted);gap:10px;padding:10px 3px 2px}.hero-card figcaption span:first-child{font-family:Georgia,serif;font-size:16px;color:var(--ink)}.hero-card:nth-child(2){width:55%;position:absolute;right:10px;bottom:-5px;transform:rotate(6deg);z-index:2}.hero-card:nth-child(2) figcaption span:last-child{display:none}.margin-note{position:absolute;bottom:15px;left:24px;color:var(--accent);font-family:Georgia,serif;font-style:italic;transform:rotate(-6deg);font-size:19px;line-height:1.3}.capture-note{display:flex;gap:20px;align-items:center;justify-content:space-between;padding:21px 0;border-top:1px solid var(--line);border-bottom:1px solid var(--line);font-size:11px}.capture-note p{color:var(--muted)}.capture-note>span:last-child{white-space:nowrap;color:var(--muted)}.eyebrow{font-size:10px;text-transform:uppercase;letter-spacing:.13em;font-weight:700;line-height:1.5}.animal-section{padding-top:86px;padding-bottom:70px;border-bottom:1px solid var(--line)}.section-heading{display:grid;grid-template-columns:80px 1fr;gap:23px;margin-bottom:30px}.section-number{font-family:Georgia,serif;font-size:61px;line-height:1;color:#a6ac97;border-top:2px solid var(--olive);padding-top:12px}.heading-line{display:flex;justify-content:space-between;align-items:center;gap:20px;margin-bottom:11px}.heading-line .eyebrow{color:var(--accent)}.status-pill{border:1px solid #b9c0ad;border-radius:30px;padding:6px 10px;color:var(--olive);font-size:10px;line-height:1.4}.section-heading h2{font-size:clamp(42px,5vw,66px);letter-spacing:-.04em;line-height:1.05}.section-description{max-width:780px;font-size:14px;color:var(--muted);margin-top:17px}.focus-list{display:flex;flex-wrap:wrap;gap:7px 20px;margin-top:20px;font-size:11px}.focus-list span:before{content:"+";color:var(--accent);margin-right:7px}.comparison{border:1px solid var(--line);background:var(--card);border-radius:var(--radius);overflow:hidden}.viewer-toolbar{padding:16px 22px;display:flex;align-items:center;justify-content:space-between;gap:20px}.view-tabs{display:flex;gap:4px;flex-wrap:wrap}.view-tab{font-size:11px;padding:8px 12px;background:transparent;border-radius:3px}.view-tab[aria-pressed=true]{background:var(--ink);color:var(--card)}.view-tab:hover:not([aria-pressed=true]){background:#e8e6da}.compare-stage{position:relative;aspect-ratio:4/3;max-height:730px;overflow:hidden;background:#1b2022}.compare-stage>img,.compare-before img{height:100%;width:100%;object-fit:contain}.compare-before{position:absolute;inset:0;clip-path:inset(0 calc(100% - var(--split)) 0 0)}.compare-line{position:absolute;top:0;bottom:0;left:var(--split);width:2px;background:#fbf7ef;box-shadow:0 0 2px #000;pointer-events:none}.compare-line span{position:absolute;top:50%;left:50%;display:grid;place-items:center;width:42px;height:42px;transform:translate(-50%,-50%);border-radius:50%;background:var(--card);color:var(--ink);font-size:22px;box-shadow:0 1px 12px #0003}.is-paired .compare-stage{cursor:ew-resize;touch-action:pan-y}.image-label{position:absolute;top:17px;padding:7px 10px;background:#1c2824dc;border:1px solid #ffffff2e;color:#fff;font-size:10px;border-radius:3px;pointer-events:none}.original-label{left:18px}.current-label{right:18px}.compare-control{padding:20px 24px 10px;border-bottom:1px solid var(--line)}.compare-control label{display:flex;justify-content:space-between;font-size:12px;font-weight:650}.compare-control label span{font-weight:400;color:var(--muted);font-size:11px}input[type=range]{width:100%;accent-color:var(--olive);display:block;margin:15px 0 9px;height:18px;cursor:ew-resize}.range-ends{display:flex;justify-content:space-between;font-size:10px;color:var(--muted);margin-bottom:6px}.viewer-footer{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:13px 22px;font-size:10px;color:var(--muted)}.viewer-footer>div{display:flex;gap:20px}.text-button{font-size:10px;padding:4px 0;background:transparent;text-decoration:underline;text-underline-offset:3px}.baseline-note{padding:16px 22px;font-size:12px;color:var(--muted);border-bottom:1px solid var(--line)}.subheading{display:flex;justify-content:space-between;align-items:end;gap:20px;margin:37px 0 18px}.subheading h3{font-size:27px;letter-spacing:-.02em}.subheading>span{font-size:10px;color:var(--muted)}.stage-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:18px}.stage-card{border:1px solid var(--line);background:var(--card);border-radius:var(--radius);overflow:hidden}.stage-top{display:flex;justify-content:space-between;gap:10px;padding:14px 14px 12px;align-items:center}.stage-top .eyebrow{font-size:9px}.small-status{font-size:9px;color:var(--olive);background:#e7eadd;padding:4px 7px;border-radius:3px;white-space:nowrap}.image-button{display:block;padding:0;background:#1b2022;position:relative;width:100%;overflow:hidden}.image-button img{height:100%;object-fit:contain}.zoom-cue{position:absolute;bottom:9px;right:9px;width:27px;height:27px;border-radius:50%;background:#faf7efed;color:var(--ink);display:grid;place-items:center;font-size:16px;opacity:0;transition:opacity .15s}.image-button:hover .zoom-cue,.image-button:focus-visible .zoom-cue{opacity:1}.stage-copy{padding:18px 19px 21px}.stage-copy h4{font-size:22px;letter-spacing:-.02em}.stage-copy p,.stage-copy li{font-size:12px;color:var(--muted)}.stage-copy p{margin-top:10px}.stage-copy ul{margin:12px 0 0}.motion-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:24px}.motion-card{min-width:0;background:var(--card);padding:20px;border:1px solid var(--line);border-radius:var(--radius)}.motion-heading{display:flex;gap:14px;justify-content:space-between;align-items:center}.motion-heading h4{font-size:25px;letter-spacing:-.02em}.motion-card>p{font-size:12px;color:var(--muted);margin:10px 0 16px;max-width:660px}.motion-card video{aspect-ratio:4/3;background:#1b2022;border-radius:2px}.filmstrip{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:4px;padding-top:12px}.filmstrip figure{min-width:0}.filmstrip .image-button{border-radius:2px}.filmstrip .zoom-cue{display:none}.filmstrip figcaption{font-variant-numeric:tabular-nums;font-size:8px;text-align:center;color:var(--muted);padding-top:5px}.motion-card .motion-note{font-size:10px;line-height:1.65;margin:14px 0 0;padding-top:12px;border-top:1px solid var(--line)}.animal-caveat{margin-top:24px;border-left:2px solid var(--accent);padding:3px 0 3px 18px;display:grid;grid-template-columns:120px 1fr;gap:15px}.animal-caveat strong{font-size:11px;color:var(--accent);padding-top:2px}.animal-caveat ul{margin:0;font-size:11px;color:var(--muted)}
.performance-section{display:grid;grid-template-columns:1fr 1.65fr;gap:45px;padding:60px 30px;margin:45px -30px;background:var(--dark);color:var(--paper);border-radius:5px}.performance-section .eyebrow{color:#c3cfae}.performance-section h2{font-size:43px;margin:12px 0 20px;letter-spacing:-.035em}.performance-section p{font-size:12px;color:#c5cabc}.cpu-label{display:inline-block;margin-top:22px;font-size:10px;border:1px solid #71816a;border-radius:20px;padding:6px 10px;color:#d8dec9}.performance-data{min-width:0}.table-scroll{overflow-x:auto}table{border-collapse:collapse;width:100%;font-size:12px;text-align:left}thead th{font-size:9px;font-weight:400;color:#b9c4ae;padding:0 10px 17px;white-space:nowrap}thead th:first-child{padding-left:0}tbody th,td{border-top:1px solid #53614f;padding:19px 10px;font-variant-numeric:tabular-nums;vertical-align:top}tbody th{font-weight:600;font-size:12px;padding-left:0}tbody th span{display:block;font-size:9px;font-weight:400;color:#b9c4ae;margin-top:7px;white-space:nowrap}td{font-size:21px;font-family:Georgia,serif}td small{display:block;font-family:system-ui,sans-serif;font-size:9px;color:#b9c4ae;margin-top:8px;white-space:nowrap}.geometry-counts{display:flex;gap:8px 20px;flex-wrap:wrap;border-top:1px solid #53614f;padding-top:17px;font-size:10px;color:#c5cabc}.geometry-counts strong{color:var(--paper)}details{margin-top:17px}summary{cursor:pointer;font-size:10px;text-decoration:underline;text-underline-offset:3px}details p{margin-top:12px}.evidence-section{padding:10px 0 50px}.evidence-section .subheading{margin-top:0}.evidence-section h2{font-size:33px;letter-spacing:-.025em}.process-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:40px;padding:25px 0 35px}.process-grid article{border-top:1px solid var(--line);padding-top:16px}.process-grid .eyebrow{color:var(--accent)}.process-grid h3{font-size:25px;margin:11px 0}.process-grid p{font-size:12px;color:var(--muted)}.evidence-bottom{display:grid;grid-template-columns:1fr 1fr;gap:70px;padding:30px 0 10px;border-top:1px solid var(--line)}.evidence-bottom h3{font-size:22px;margin-bottom:14px}.evidence-bottom p,.evidence-bottom li{font-size:11px;color:var(--muted)}.evidence-bottom ul{margin:0}.source-link{display:block;text-decoration:none;font-size:11px;margin-top:18px}.source-link strong{font-weight:600;text-decoration:underline;text-underline-offset:4px}.source-link span{display:block;font-size:10px;color:var(--muted);margin-top:7px;line-height:1.6}footer{display:flex;justify-content:space-between;gap:24px;border-top:1px solid var(--line);padding:25px max(28px,calc((100vw - 1264px)/2));font-size:9px;color:var(--muted)}footer a{text-decoration:none}.noscript{padding:20px;background:#fff0d4;color:var(--ink)}
dialog{padding:0;border:1px solid #71796b;border-radius:5px;background:#142019;color:#f3eee4;width:min(96vw,1500px);max-width:96vw;max-height:94vh;overflow:hidden}dialog::backdrop{background:#101a17ed;backdrop-filter:blur(6px)}.lightbox-bar{display:flex;align-items:center;justify-content:space-between;gap:20px;padding:16px 20px;font-size:12px}.lightbox-bar button{background:#344334;color:#f3eee4;border:1px solid #67735d;padding:9px 13px;border-radius:3px;font-size:11px}.lightbox-bar button span{font-size:17px;margin-left:10px}.lightbox-image{max-height:76vh;overflow:auto;background:#1b2022}.lightbox-image img{width:100%;height:100%;max-height:76vh;object-fit:contain;cursor:zoom-in}.lightbox-image.is-full img{width:auto;max-width:none;height:auto;max-height:none;margin:auto;cursor:zoom-out}.lightbox-help{font-size:10px;color:#b9c4ae;padding:12px 20px}body.modal-open{overflow:hidden}
@media(min-width:1000px){.stage-grid:has(.stage-card:only-child){grid-template-columns:minmax(300px,1fr) 1fr 1fr}.motion-grid:has(.motion-card:only-child){grid-template-columns:1fr 1fr}.motion-grid .motion-card:only-child{grid-column:1/-1;display:grid;grid-template-columns:1.1fr 1fr;column-gap:26px}.motion-card:only-child .motion-heading{grid-column:2;grid-row:1;align-self:start}.motion-card:only-child>p:not(.motion-note){grid-column:2;grid-row:2;align-self:start;margin-top:0}.motion-card:only-child video{grid-column:1;grid-row:1/6}.motion-card:only-child .filmstrip{grid-column:2;grid-row:3;align-self:end}.motion-card:only-child .motion-note{grid-column:2;grid-row:4}}
@media(max-width:1000px){.hero{min-height:560px;gap:20px}.hero-collage{min-height:410px;padding-left:0;padding-right:5px}.hero-card{width:94%}.hero-card:nth-child(2){width:68%;bottom:0}.hero h1{font-size:68px}.edition{gap:10px;font-size:9px}.heading-line{align-items:start}.section-heading{grid-template-columns:58px 1fr;gap:19px}.section-number{font-size:46px}.stage-grid{gap:12px}.stage-top{display:block}.small-status{display:inline-block}.stage-top .small-status{margin-top:7px}.stage-copy{padding:15px}.performance-section{gap:25px;grid-template-columns:1fr 1.6fr;margin:35px 0;padding:35px 22px}.capture-note>span:last-child{display:none}.motion-card{padding:15px}.motion-heading{align-items:start}.motion-heading h4{font-size:22px}}
@media(max-width:760px){.site-header{height:auto;min-height:73px;padding:13px 20px;gap:12px;flex-wrap:wrap}.wordmark{font-size:11px}.wordmark-sub{font-size:13px}.wordmark-mark{font-size:26px}.site-header nav{gap:18px;font-size:10px}.hero{grid-template-columns:1fr;min-height:0;padding:40px 0 35px;gap:30px}.hero h1{font-size:65px;max-width:440px}.hero-intro{max-width:550px;font-size:14px}.hero-collage{min-height:440px;max-width:520px;width:100%;margin:auto;padding-left:20px;padding-right:20px}.hero-card{width:82%}.hero-card:nth-child(2){width:62%;right:20px;bottom:5px}.margin-note{left:28px}.capture-note{display:block;padding:17px 0}.capture-note p{margin-top:6px}.section-heading{grid-template-columns:42px 1fr;gap:16px}.section-number{font-size:34px}.heading-line{display:block}.heading-line .status-pill{display:inline-block;margin-top:9px}.section-heading h2{font-size:47px}.animal-section{padding-top:50px;padding-bottom:40px}.section-description{font-size:13px}.focus-list{font-size:10px;gap:7px 12px}.viewer-toolbar{display:block;padding:15px}.view-tabs{margin-top:10px;gap:2px}.view-tab{padding:8px;font-size:10px}.compare-control{padding:17px 15px 10px}.compare-control label span{font-size:9px}.viewer-footer{padding:12px 15px;display:block}.viewer-footer>div{margin-top:5px;gap:20px}.compare-line span{width:33px;height:33px;font-size:19px}.image-label{font-size:8px;top:10px;padding:5px 7px}.original-label{left:10px}.current-label{right:10px}.subheading{display:block;margin-top:27px}.subheading>span{display:block;margin-top:8px}.subheading h3{font-size:25px}.stage-grid{grid-template-columns:1fr 1fr;gap:15px}.stage-card:first-child:last-child{grid-column:1/-1;max-width:450px}.stage-copy h4{font-size:22px}.motion-grid{grid-template-columns:1fr}.motion-heading{align-items:center}.motion-card{padding:17px}.filmstrip figcaption{font-size:9px}.animal-caveat{display:block}.animal-caveat ul{margin-top:8px}.performance-section{grid-template-columns:1fr;gap:28px;padding:30px 20px}.performance-section h2{font-size:37px}.process-grid{grid-template-columns:1fr;gap:22px;padding-top:15px}.process-grid article{display:grid;grid-template-columns:26px 1fr;column-gap:13px}.process-grid h3{margin:0 0 8px}.process-grid p{grid-column:2}.process-grid .eyebrow{margin-top:7px}.evidence-bottom{grid-template-columns:1fr;gap:30px}.evidence-section h2{font-size:29px}footer{padding:23px 20px;flex-wrap:wrap;font-size:9px;gap:14px}footer span:nth-child(2){order:3;width:100%}footer a{margin-left:auto}main{padding:0 20px}.zoom-cue{opacity:1;width:24px;height:24px;font-size:14px}.filmstrip .zoom-cue{display:none}}
@media(max-width:440px){.site-header nav{width:100%;justify-content:space-between}.hero h1{font-size:55px}.hero-collage{min-height:350px;padding-left:6px;padding-right:6px}.hero-card{width:87%}.hero-card:nth-child(2){right:5px;bottom:0;width:66%}.margin-note{font-size:15px;left:8px;bottom:5px}.edition{font-size:8px;gap:13px}.hero-intro{font-size:13px}.stage-grid{grid-template-columns:1fr}.stage-top{display:flex}.stage-top .small-status{margin-top:0}.section-heading{grid-template-columns:32px 1fr;gap:13px}.section-heading h2{font-size:41px}.section-number{font-size:29px}.focus-list{display:grid;gap:6px}.view-tab{padding:7px 6px;font-size:9px}.compare-control label{font-size:11px}.compare-control label span{font-size:8px}.range-ends{font-size:8px}.performance-section{padding:26px 14px}thead th{font-size:8px;padding:0 7px 13px;white-space:normal}tbody th{font-size:10px}tbody th span{font-size:8px;white-space:normal}td{font-size:18px;padding:16px 7px}td small{font-size:8px}.lightbox-bar{padding:11px}.lightbox-bar p{font-size:10px}.lightbox-bar button{white-space:nowrap}}
@media(prefers-reduced-motion:reduce){html{scroll-behavior:auto}*{transition:none!important}}
@media print{.site-header{position:static}nav,.start-link,.compare-control,.viewer-footer,video,.zoom-cue,footer a{display:none!important}.hero{min-height:0;padding-top:30px}.animal-section{break-before:page}.stage-card,.motion-card{break-inside:avoid}.compare-stage{max-height:450px}.performance-section{color:#000;background:#eee}.performance-section p,.performance-section .eyebrow,.cpu-label,td small,thead th,tbody th span{color:#333}.stage-grid{grid-template-columns:repeat(3,1fr)}.evidence-section{break-before:page}}
'''

JS = r'''
(() => {
  'use strict';
  const media = JSON.parse(document.getElementById('media-data').textContent);
  const comparisons = JSON.parse(document.getElementById('comparison-data').textContent);
  document.querySelectorAll('[data-media]').forEach(el => { el.src = media[el.dataset.media]; });
  document.querySelectorAll('[data-poster]').forEach(el => { el.poster = media[el.dataset.poster]; });
  document.querySelectorAll('[data-comparison]').forEach(viewer => {
    const id = viewer.dataset.comparison;
    const stage = viewer.querySelector('.compare-stage');
    const range = viewer.querySelector('input[type=range]');
    const change = value => {
      const amount = Math.max(0, Math.min(100, Math.round(value)));
      range.value = amount;
      range.setAttribute('aria-valuetext', `${amount} percent original, ${100-amount} percent revised`);
      stage.style.setProperty('--split', amount + '%');
    };
    if (range) {
      range.addEventListener('input', () => change(range.value));
      let dragging = false;
      const position = e => { const r = stage.getBoundingClientRect(); change((e.clientX-r.left)/r.width*100); };
      stage.addEventListener('pointerdown', e => {
        if (e.button !== 0) return;
        dragging = true; stage.setPointerCapture(e.pointerId); position(e);
      });
      stage.addEventListener('pointermove', e => { if (dragging) position(e); });
      stage.addEventListener('pointerup', () => { dragging = false; });
      stage.addEventListener('pointercancel', () => { dragging = false; });
    }
    viewer.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', () => {
      const view = comparisons[id][button.dataset.view];
      viewer.dataset.currentView = button.dataset.view;
      viewer.querySelectorAll('[data-view]').forEach(other => other.setAttribute('aria-pressed', String(other === button)));
      const after = viewer.querySelector('.compare-after');
      after.src = media[view.after]; after.alt = `${id} ${range ? "revision" : "original"} · ${view.label}`;
      const before = viewer.querySelector('.compare-before img');
      if (before) { before.src = media[view.before]; before.alt = `${id} original · ${view.label}`; }
      viewer.querySelector('.open-before').dataset.zoom = view.before;
      const afterButton = viewer.querySelector('.open-after');
      if (afterButton) afterButton.dataset.zoom = view.after;
      viewer.querySelector('.view-status').textContent = `${view.label} · matched studio camera`;
    }));
  });
  const dialog = document.getElementById('lightbox');
  const fullImage = document.getElementById('lightbox-img');
  const caption = document.getElementById('lightbox-caption');
  const imageWrap = dialog.querySelector('.lightbox-image');
  document.addEventListener('click', e => {
    const button = e.target.closest('[data-zoom]');
    if (!button) return;
    fullImage.src = media[button.dataset.zoom];
    const viewer = button.closest('[data-comparison]');
    const view = viewer ? comparisons[viewer.dataset.comparison][viewer.dataset.currentView].label : '';
    fullImage.alt = button.dataset.caption + (view ? ` · ${view}` : '');
    caption.textContent = fullImage.alt;
    imageWrap.classList.remove('is-full');
    document.body.classList.add('modal-open');
    dialog.showModal();
  });
  document.getElementById('lightbox-close').addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', () => document.body.classList.remove('modal-open'));
  dialog.addEventListener('click', e => { if (e.target === dialog) dialog.close(); });
  fullImage.addEventListener('click', () => imageWrap.classList.toggle('is-full'));
  document.querySelectorAll('video').forEach(video => video.addEventListener('play', () => {
    document.querySelectorAll('video').forEach(other => { if (other !== video) other.pause(); });
  }));
})();
'''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--manifest", type=Path, default=ROOT / "gauntlet/animal_gallery.json")
    parser.add_argument("--output", type=Path, default=ROOT / "gauntlet/animals.html")
    parser.add_argument("--linked", action="store_true", help="Keep images/videos as relative paths instead of embedding")
    parser.add_argument("--check", action="store_true", help="Validate the manifest and all referenced files without writing")
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text())
        gallery = Gallery(manifest, args.output.resolve(), args.linked)
        document = gallery.build()
        if not args.check:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(document, encoding="utf-8")
        verb = "Validated" if args.check else "Built"
        print(f"{verb} {args.output}: {len(gallery.paths)} unique media files, {len(document.encode('utf-8')) / 1024 / 1024:.1f} MiB HTML")
    except (OSError, ValueError, KeyError, StopIteration, TypeError) as error:
        print(f"Gallery build failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
