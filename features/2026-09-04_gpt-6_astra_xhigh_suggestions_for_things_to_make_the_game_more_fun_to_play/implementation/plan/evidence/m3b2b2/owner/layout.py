"""Inspect final test ELF layouts without running or copying the images."""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path.cwd()
name = sys.argv[1]
log = Path('/tmp/alibi-m3b2b2-' + name + '.log').read_text()


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


types = {
    'cathedral_sim': [
        'prompt_archive::PromptExchange',
        'prompt_archive::PromptExchangeData',
        'prompt_archive::PromptArchivePermit',
        'checkpoint::budget::Usage',
        'checkpoint::budget::Group',
        'checkpoint::budget::Reservation',
        'checkpoint::budget::PromotionPermit',
        'checkpoint::budget::RetirementOwner',
        'checkpoint::budget::RetirementRelease',
    ],
    'cathedral_backends': [
        'runtime::BackendRuntime',
        'runtime::BackendExecutor',
        'dns::DnsPool',
        'dns::NativeResolver',
        'dns::DnsPermit',
        'dns::Answer',
        'dns::DnsWork',
        'transcription::SttEngine',
        'transcription::Job',
        'transcription::Discard',
        'tts::TtsEngine',
        'tts::Job',
        'worker::Worker',
        'worker::Children',
        'worker::NativeChild',
        'worker::WorkerIo',
        'stt_realtime::SessionTask',
        'stt_realtime::RetainedClose',
        'prompt_log::Core',
        'prompt_log::PayloadCharge',
        'prompt_log::Receipt',
        'prompt_log::Order',
        'prompt_log::Progress',
        'prompt_log::Session',
        'prompt_log::WriteJob',
        'prompt_log::WriterOwner',
        'prompt_log::ArchiveWriter',
        'prompt_log::PromptLog',
        'checkpoint_preparation::Core',
        'checkpoint_preparation::Queue',
        'checkpoint_preparation::PrepState',
        'checkpoint_preparation::PreparedDelivery',
        'checkpoint_preparation::DeliveryDisposal',
        'checkpoint_preparation::RetiredPayload',
        'checkpoint_preparation::PreparationPermit',
        'checkpoint_preparation::RetirementPermit',
        'checkpoint_preparation::Job',
    ],
}
images = {}
for crate, names in types.items():
    found = re.findall(
        r'Running unittests src/lib\.rs \((target/debug/deps/' + crate + r'-[a-f0-9]+)\)',
        log,
    )
    assert len(found) == 1, found
    image = ROOT / found[0]
    before = sha(image)
    command = ['/usr/bin/gdb', '-nx', '-nh', '--batch', str(image), '-ex', 'set language rust']
    for typename in names:
        command += ['-ex', 'p sizeof(' + crate + '::' + typename + ')']
    result = subprocess.run(command, text=True, capture_output=True)
    print(result.stdout, end='')
    print(result.stderr, end='')
    assert result.returncode == 0, result.returncode
    sizes = [int(v) for v in re.findall(r'^\$\d+ = (\d+)$', result.stdout, re.M)]
    assert len(sizes) == len(names), (names, sizes)
    assert sha(image) == before, 'ELF changed during read-only inspection'
    images[crate] = {
        'path': str(image), 'sha256': before, 'bytes': image.stat().st_size,
        'layout': dict(zip(names, sizes)), 'command': command, 'target_executed': False,
    }

s = images['cathedral_sim']['layout']
b = images['cathedral_backends']['layout']
control = (
    b['checkpoint_preparation::Core']
    + 4 * max(b['checkpoint_preparation::' + typename] for typename in (
        'PrepState', 'PreparedDelivery', 'DeliveryDisposal', 'RetiredPayload',
    ))
    + b['checkpoint_preparation::PreparationPermit']
    + b['checkpoint_preparation::RetirementPermit']
    + b['checkpoint_preparation::Job']
    + s['checkpoint::budget::Usage']
    + 2 * s['checkpoint::budget::RetirementOwner']
    + 4 * s['checkpoint::budget::RetirementRelease']
    + 3 * s['checkpoint::budget::PromotionPermit']
    + 1024  # Arc controls, fixed group pairs, allocation rounding.
)
assert control < 64 * 1024, control
record = {
    'images': images, 'helper_sha256': sha(Path(__file__)),
    'fixed_control_upper_bytes': control, 'fixed_control_allowance_bytes': 64 * 1024,
    'source_map_sha256': json.loads((HERE / (name + '-start.json')).read_text())['source_map_sha256'],
    'binary_copies_preserved': False,
}
(HERE / 'final-images.json').write_text(json.dumps(record, sort_keys=True, indent=2) + '\n')
print(json.dumps(record, sort_keys=True))
