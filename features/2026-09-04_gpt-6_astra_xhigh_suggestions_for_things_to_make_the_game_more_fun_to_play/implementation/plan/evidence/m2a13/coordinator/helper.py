from pathlib import Path
p=Path('crates/cathedral-sim/src/receipts/checkpoint.rs')
s=p.read_text(); cut=s.index('\n/// Borrowed receipt projection for the interrupted-speech owner.')
part=s[cut:]
p.write_text(s[:cut].replace('pub(crate) struct EntryV1 {','struct EntryV1 {')+'\nmod speech;\npub(crate) use speech::CheckpointReceiptRef;\n')
p=Path('crates/cathedral-sim/src/receipts/checkpoint/speech.rs');p.parent.mkdir(exist_ok=True)
p.write_text('use super::*;\n'+part.replace('pub(crate) enum CheckpointReceiptRef', '#[allow(private_interfaces)]\npub(crate) enum CheckpointReceiptRef'))
