from pathlib import Path
p=Path('crates/cathedral-sim/src/speech_router/checkpoint/records.rs');s=p.read_text()
for name,adapter in [('InterruptedStream','stream'),('AcceptedRecording','recording')]:
 start=s.index('#[derive(Debug, Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct '+name)
 end=s.index('\nimpl '+name,start)
 block=s[start:end]
 public=block.replace('Debug, Serialize, Deserialize','Debug, Serialize').replace('#[serde(deny_unknown_fields)]\n','')
 remote=block.replace('#[derive(Debug, Serialize, Deserialize)]','# [derive(Serialize, Deserialize)]').replace('#[serde(deny_unknown_fields)]',f'#[serde(remote="{name}",deny_unknown_fields)]').replace('pub struct '+name,'struct '+name+'V1').replace('pub(super) ','')
 s=s[:start]+public+'\n'+remote+f'\nremote_adapters!({adapter}, {name}, {name}V1);'+s[end:]
s=s.replace('    pub streams: Vec<InterruptedStream>,','    #[serde(with="stream::vec")]\n    pub streams: Vec<InterruptedStream>,').replace('    pub accepted_recordings: Vec<AcceptedRecording>,','    #[serde(with="recording::vec")]\n    pub accepted_recordings: Vec<AcceptedRecording>,')
p.write_text(s)
p=Path('crates/cathedral-sim/src/speech_router/checkpoint.rs');s=p.read_text()
start=s.index('    crate::checkpoint::logical(OWNER,r.at)?;');end=s.index('    check(c.receipt(r.id)',start)
s=s[:start]+'    crate::receipts::validate_speech_receipt(r,c.now)?;\n'+s[end:]
s=s.replace('check(c.receipt(id).is_some(),"speech task receipt/root disagreement")?;', 'let receipt=c.receipt(id).ok_or_else(||CheckpointError::new(OWNER,"speech task receipt/root disagreement"))?;\n        receipt.validate(c.now)?;')
p.write_text(s)
p=Path('crates/cathedral-sim/src/receipts/checkpoint/speech.rs');s=p.read_text();s=s.replace('    pub(crate) fn copy(&self)->Receipt {','    pub(crate) fn validate(&self,now:LogicalTime)->Result<()> {\n        match self { Self::Live(r)=>validate_speech_receipt(r,now),Self::Saved(e)=>e.validate(now) }\n    }\n    pub(crate) fn copy(&self)->Receipt {')
s+='''
/// Same closed Receipt field domain as EntryV1::validate, without conversion,
/// string allocation or reconstruction. Context owns payload/entry validation.
pub(crate) fn validate_speech_receipt(r:&Receipt,now:LogicalTime)->Result<()> {
    checkpoint::logical(OWNER,r.at)?;
    if r.at>now.seconds() || r.ordinal==0 || r.outcome.code.len()>48
        || !name(&r.outcome.code) || r.outcome.code=="dispatch_pending"
        || r.outcome.message.len()>192 || r.outcome.message.chars().any(char::is_control)
        || r.affected.len()>2 {return Err(err("invalid speech receipt fields"));}
    for a in &r.affected {
        if a.kind.len()>16 || a.id.len()>64 || !name(&a.id)
            || !matches!(a.kind.as_str(),"actor"|"item"|"fixture"|"mark"|"ward")
            || (a.kind=="mark" && a.id.parse::<u64>().is_err())
            || (a.kind=="ward" && !crate::lore::PlanningWard::ALL.iter().any(|w|w.as_str()==a.id))
        {return Err(err("invalid speech receipt principal reference"));}
    }
    Ok(())
}
''';p.write_text(s)
p=Path('crates/cathedral-sim/src/receipts/checkpoint.rs');s=p.read_text().replace('pub(crate) use speech::CheckpointReceiptRef;', 'pub(crate) use speech::{CheckpointReceiptRef,validate_speech_receipt};');p.write_text(s)
p=Path('crates/cathedral-sim/src/receipts.rs');s=p.read_text().replace('pub(crate) use checkpoint::CheckpointReceiptRef;', 'pub(crate) use checkpoint::{CheckpointReceiptRef,validate_speech_receipt};');p.write_text(s)
