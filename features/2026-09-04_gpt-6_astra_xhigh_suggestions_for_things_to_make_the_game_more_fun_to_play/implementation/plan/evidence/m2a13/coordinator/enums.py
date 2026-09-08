from pathlib import Path
p=Path('crates/cathedral-sim/src/speech_router/checkpoint/records.rs');s=p.read_text()
s=s.replace('    state: ReceiptState,','    #[serde(deserialize_with="receipt_state")]\n    state: ReceiptState,')
s=s.replace('    pub(super) backend: SttBackendKind,','    #[serde(deserialize_with="backend")]\n    pub(super) backend: SttBackendKind,')
s=s.replace('    backend: SttBackendKind,','    #[serde(deserialize_with="backend")]\n    backend: SttBackendKind,')
s+='''
fn backend<'de,D:serde::Deserializer<'de>>(d:D)->std::result::Result<SttBackendKind,D::Error>{
    struct V;
    impl serde::de::Visitor<'_> for V {
        type Value=SttBackendKind;
        fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("cloud or local")}
        fn visit_str<E:serde::de::Error>(self,s:&str)->std::result::Result<Self::Value,E>{match s{"cloud"=>Ok(SttBackendKind::Cloud),"local"=>Ok(SttBackendKind::Local),_=>Err(E::custom("invalid STT backend"))}}
    }
    d.deserialize_str(V)
}
fn receipt_state<'de,D:serde::Deserializer<'de>>(d:D)->std::result::Result<ReceiptState,D::Error>{
    struct V;
    impl serde::de::Visitor<'_> for V {
        type Value=ReceiptState;
        fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("a canonical receipt state string")}
        fn visit_str<E:serde::de::Error>(self,s:&str)->std::result::Result<Self::Value,E>{match s{
            "rejected"=>Ok(ReceiptState::Rejected),"accepted"=>Ok(ReceiptState::Accepted),"in_progress"=>Ok(ReceiptState::InProgress),"completed"=>Ok(ReceiptState::Completed),"interrupted"=>Ok(ReceiptState::Interrupted),"superseded"=>Ok(ReceiptState::Superseded),_=>Err(E::custom("invalid receipt state"))}}
    }
    d.deserialize_str(V)
}
'''
p.write_text(s)
p=Path('crates/cathedral-sim/src/speech_router/checkpoint/tests.rs');s=p.read_text();needle='("/state/accepted_recordings/0/source", "batch_pending"),';s=s.replace(needle,needle+'\n        ("/state/accepted_recordings/0/backend", "cloud"),\n        ("/state/accepted_recordings/0/receipt/outcome/state", "accepted"),');p.write_text(s)
