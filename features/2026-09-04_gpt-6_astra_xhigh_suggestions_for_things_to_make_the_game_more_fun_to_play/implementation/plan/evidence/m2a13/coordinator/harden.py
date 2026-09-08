from pathlib import Path
p=Path('crates/cathedral-sim/src/speech_router/checkpoint/records.rs');s=p.read_text()
for name in ('InputPurpose','InterruptionStatus','RecordingSource'):
 # Enums have only the canonical string representation, never serde's unit-map alias.
 marker='#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(rename_all="snake_case")]\npub enum '+name
 s=s.replace(marker,marker.replace(', Deserialize',''))
s+='''
macro_rules! closed_string_enum {
    ($ty:ident, $($text:literal => $value:path),+ $(,)?) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{
                struct V;
                impl serde::de::Visitor<'_> for V {
                    type Value=$ty;
                    fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("a canonical speech enum string")}
                    fn visit_str<E:serde::de::Error>(self,v:&str)->std::result::Result<$ty,E>{match v {$($text=>Ok($value),)+_=>Err(E::custom("unknown speech variant"))}}
                }
                d.deserialize_str(V)
            }
        }
    };
}
closed_string_enum!(InputPurpose,"public_player_speech"=>InputPurpose::PublicPlayerSpeech);
closed_string_enum!(InterruptionStatus,"interrupted_unsent"=>InterruptionStatus::InterruptedUnsent);
closed_string_enum!(RecordingSource,"batch_pending"=>RecordingSource::BatchPending,"parked"=>RecordingSource::Parked);
'''
p.write_text(s)
p=Path('crates/cathedral-sim/src/speech_router/checkpoint.rs');s=p.read_text()
s=s.replace('    basename(&t.basename)?;','    let TranscriptionTask {semantic:_,request_id:_,basename:_,position_m:_,backend:_,attention:_}=t;\n    basename(&t.basename)?;',1)
s=s.replace('            basename(key)?;check(r.streams', '            let StreamState {phase:_,next_seq:_,decoded_bytes:_,end_at:_,commit_at:_,completed_at:_,transcript:_,degrade_reason:_,status_sent:_}=s;\n            basename(key)?;check(r.streams')
p.write_text(s)
p=Path('crates/cathedral-sim/src/speech_router/checkpoint/tests.rs');s=p.read_text().replace('assert_eq!(decode(&raw,&w).unwrap().encode().unwrap().value(),&raw);','assert_eq!(serde_json::to_vec(decode(&raw,&w).unwrap().value()).unwrap(),raw);')
s+='''
#[test]
fn checkpoint_speech_enum_type_receipt_principal_and_preflight_domain(){
    let(r,mut w)=fixture();let v:Value=serde_json::from_slice(&bytes(&r,&w)).unwrap();
    for(pointer,key)in [("/state/purpose","public_player_speech"),("/state/status","interrupted_unsent"),("/state/accepted_recordings/0/source","batch_pending")]{let mut bad=v.clone();*bad.pointer_mut(pointer).unwrap()=json!({key:null});assert!(decode(&serde_json::to_vec(&bad).unwrap(),&w).is_err());}
    let id=r.recording_jobs[0].1.semantic.unwrap();
    for (kind,key) in [("mark","not-a-number"),("ward","not-a-ward")]{
        w.command_ledger.recent.get_mut(&id).unwrap().receipt.affected=vec![crate::receipts::AffectedRef{kind:kind.into(),id:key.into()}];
        assert!(r.checkpoint_cost(context(&w),save()).is_err());assert!(r.export_checkpoint(context(&w),save()).is_err());
    }
}
#[test]
fn checkpoint_speech_new_fixtures_pin_projection(){
    let(r,w)=fixture();let base=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert_eq!(std::fs::read(base.join("checkpoint_speech_pending_v1.json")).unwrap(),bytes(&r,&w));
    let w=World::new();assert_eq!(std::fs::read(base.join("checkpoint_speech_empty_v1.json")).unwrap(),bytes(&SpeechRouter::default(),&w));
}
'''
p.write_text(s)
p=Path('crates/cathedral-sim/src/engine/speech_checkpoint/tests.rs');s=p.read_text()+'''
#[test]
fn checkpoint_speech_engine_fixtures_pin_projection(){
    let mut e=engine();let base=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert_eq!(std::fs::read(base.join("checkpoint_engine_speech_empty_v1.json")).unwrap(),bytes(&e));
    e.poll(0.0,vec![EngineCommand::PlayerUtteranceStarted{wav_basename:"onset.wav".into()}]);
    assert_eq!(std::fs::read(base.join("checkpoint_engine_speech_onset_v1.json")).unwrap(),bytes(&e));
}
''';p.write_text(s)
