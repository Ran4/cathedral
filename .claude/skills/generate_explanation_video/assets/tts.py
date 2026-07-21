# /// script
# requires-python = ">=3.11"
# dependencies = ["requests"]
# ///
"""Synthesize per-scene narration with ElevenLabs and measure exact durations."""
import json, os, subprocess, sys
from pathlib import Path
import requests

HERE = Path(__file__).parent

# Load ELEVENLABS_API_KEY from the environment, or from a .env file. Point
# CATHEDRAL_ENV at your .env, else we walk up from CWD looking for one (the repo
# root .env holds ELEVENLABS_API_KEY / OPENAI_API_KEY).
def _load_env():
    if os.environ.get("ELEVENLABS_API_KEY"):
        return
    cand = []
    if os.environ.get("CATHEDRAL_ENV"):
        cand.append(Path(os.environ["CATHEDRAL_ENV"]))
    d = Path.cwd()
    for p in [d, *d.parents]:
        cand.append(p / ".env")
    for env in cand:
        if env.is_file():
            for line in env.read_text().splitlines():
                if "=" in line and not line.strip().startswith("#"):
                    k, v = line.split("=", 1)
                    os.environ.setdefault(k.strip(), v.strip())
            if os.environ.get("ELEVENLABS_API_KEY"):
                return

_load_env()
KEY = os.environ["ELEVENLABS_API_KEY"]
VOICE = "JBFqnCBsd6RMkjVDRZzb"  # George - warm, captivating storyteller (british)
MODEL = "eleven_multilingual_v2"

scenes = json.loads((HERE / "narration.json").read_text())
audio_dir = HERE / "audio"
audio_dir.mkdir(exist_ok=True)


def duration(p: Path) -> float:
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "default=nw=1:nk=1", str(p)],
        capture_output=True, text=True, check=True)
    return float(out.stdout.strip())


timings = []
for i, sc in enumerate(scenes):
    dest = audio_dir / f"{sc['id']}.mp3"
    if not dest.exists():
        body = {
            "text": sc["text"],
            "model_id": MODEL,
            "voice_settings": {
                "stability": 0.45,
                "similarity_boost": 0.75,
                "style": 0.10,
                "use_speaker_boost": True,
            },
            # prosody continuity across scene boundaries
            "previous_text": scenes[i - 1]["text"] if i > 0 else None,
            "next_text": scenes[i + 1]["text"] if i + 1 < len(scenes) else None,
        }
        r = requests.post(
            f"https://api.elevenlabs.io/v1/text-to-speech/{VOICE}",
            params={"output_format": "mp3_44100_128"},
            headers={"xi-api-key": KEY, "Content-Type": "application/json"},
            json=body, timeout=180)
        if r.status_code != 200:
            print(f"FAIL {sc['id']}: {r.status_code} {r.text[:300]}", file=sys.stderr)
            sys.exit(1)
        dest.write_bytes(r.content)
        print(f"  synth {sc['id']}  {len(r.content)/1024:.0f} KB", file=sys.stderr)
    timings.append({"id": sc["id"], "title": sc["title"], "audio": dest.name,
                    "dur": duration(dest)})

# Lay scenes out on a global timeline: lead-in, then each scene + a breath gap.
LEAD_IN = 1.2
GAP = 0.75
TAIL = 2.5
t = LEAD_IN
for e in timings:
    e["start"] = round(t, 3)
    e["end"] = round(t + e["dur"], 3)
    t += e["dur"] + GAP
total = round(t - GAP + TAIL, 3)

(HERE / "timings.json").write_text(json.dumps(
    {"lead_in": LEAD_IN, "gap": GAP, "total": total, "scenes": timings}, indent=2))

for e in timings:
    print(f"{e['id']:<18} {e['start']:>7.2f} -> {e['end']:>7.2f}  ({e['dur']:.2f}s)")
print(f"\nTOTAL {total:.1f}s  ({total/60:.1f} min)")
