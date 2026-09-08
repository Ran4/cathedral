from pathlib import Path
p=Path('crates/cathedral-backends/examples/alibi_continuity_cost.rs');s=p.read_text()
s=s.replace('M2a12 floor/Engine continuity','M2a13 interrupted speech input').replace('NullSight, NullTranscription,','NullSight,').replace('engine::continuity_checkpoint::EngineContinuityDtoV1','engine::speech_checkpoint::EngineSpeechDtoV1').replace('fake_mode: true,','fake_mode: false,\n            stt_stream_grace_seconds: 5.0,')
s=s.replace('let voices = Arc::new(std::sync::Mutex::new(Vec::new()));','let voices = Arc::new(std::sync::Mutex::new(Vec::new()));\n    let transcription=Arc::new(std::sync::Mutex::new(SttCalls::default()));')
s=s.replace('Box::new(NullTranscription)','Box::new(Stt(transcription.clone()))').replace('Capabilities::new(true, false, false, true, true, TtsBackendKind::Cloud)','Capabilities::new(true, true, true, true, true, TtsBackendKind::Cloud)')
cut=s.index('    let initial_budget =');s=s[:cut]
s+=r'''
    use cathedral_sim::{EngineCommand as Command,TranscriptionOutcome,RealtimeResult,SttBackendKind};
    use cathedral_sim::receipts::{OperationId,HOST_PRODUCER};
    let mut driver=Driver::default();
    let utterance=format!("{name}, can you tell me about this street?");
    let first=driver.poll(&mut engine,0.0,vec![Command::PlayerAttention{actor_id:Some(partner.clone())},Command::PlayerSay{request_id:"speech-first".into(),text:utterance.clone(),position_m:at,spatial_seq:1}]);
    assert_eq!(engine.scheduler().in_flight_actor_id(),Some(&partner));
    let request_id=service.lock().unwrap().pending.take().ok_or("first submission missing")?;
    let reply="say {\"target\":\"player\",\"text\":\"I can tell you about this street.\"}";
    let committed=driver.poll(&mut engine,0.05,vec![Command::LlmCompletion(cathedral_sim::Completion{request_id,result:Ok(reply.into()),duration_seconds:0.05})]);
    assert!(!voices.lock().unwrap().is_empty(),"committed NPC speech was not voiced");
    let mut submitted_inputs=Vec::new();
    macro_rules! command {
        ($time:expr,$command:expr)=>{{let cmd=$command;submitted_inputs.push(json!({"at":$time,"command":format!("{cmd:?}")}));driver.poll(&mut engine,$time,vec![cmd])}};
    }
    command!(0.10,Command::PlayerUtteranceStarted{wav_basename:"onset.wav".into()});
    command!(0.12,Command::PlayerAudioBegin{wav_basename:"available.wav".into(),sample_rate:24000});
    command!(0.14,Command::PlayerAudioChunk{wav_basename:"available.wav".into(),seq:0,samples:Arc::from([1i16,2,3,4])});
    command!(0.16,Command::PlayerAudioEnd{wav_basename:"available.wav".into(),chunk_count:1,silent:false});
    let draft_text="  Available words remain an unsent draft. A new intentional submission is required.  ";
    let draft_result=command!(0.18,Command::Transcription(TranscriptionOutcome::Realtime(RealtimeResult::Transcript{key:"available.wav".into(),text:draft_text.into()})));
    assert!(!draft_result.iter().any(|m|matches!(m,cathedral_sim::EngineMessage::Speech{..})));
    command!(0.20,Command::PlayerAudioBegin{wav_basename:"reused.wav".into(),sample_rate:24000});
    command!(0.22,Command::PlayerAudioChunk{wav_basename:"reused.wav".into(),seq:0,samples:Arc::from([1i16,2,3,4])});
    command!(0.24,Command::PlayerAudioEnd{wav_basename:"reused.wav".into(),chunk_count:1,silent:false});
    let root=OperationId{producer:HOST_PRODUCER,sequence:1};
    let parked_output=command!(0.26,Command::PlayerRecording{request_id:"reused-host-request".into(),wav_basename:"reused.wav".into(),stt_backend:SttBackendKind::Cloud,position_m:at,spatial_seq:2}.identified(root.command(0)));
    let batch_output=command!(0.28,Command::PlayerRecording{request_id:"reused-host-request".into(),wav_basename:"reused.wav".into(),stt_backend:SttBackendKind::Local,position_m:at,spatial_seq:3}.identified(root.command(1)));
    let root2=OperationId{producer:HOST_PRODUCER,sequence:2};
    let second_batch=command!(0.30,Command::PlayerRecording{request_id:"batch-second".into(),wav_basename:"batch.wav".into(),stt_backend:SttBackendKind::Cloud,position_m:at,spatial_seq:4}.identified(root2.command(0)));
    command!(0.32,Command::PlayerAudioBegin{wav_basename:"active.wav".into(),sample_rate:24000});
    command!(0.34,Command::PlayerAudioChunk{wav_basename:"active.wav".into(),seq:0,samples:Arc::from([1i16,2,3,4])});
    let boundary=LogicalTime::new(0.34).unwrap();
    let context=engine.speech_checkpoint_context(boundary);
    let service=service.lock().unwrap();let voices=voices.lock().unwrap();let transcription=transcription.lock().unwrap();
    assert_eq!(transcription.batch.len(),2);assert_eq!(engine.speech_router().parked_count(),1);assert_eq!(engine.speech_router().pending_transcription_count(),2);
    let voice_inputs:Vec<_>=voices.iter().map(|(r,accepted)|json!({"event_id":r.event_id,"text":r.text,"voice_key":r.voice_key,"kind":r.kind,"accepted":accepted})).collect();
    let receipts:Vec<_>=[root.command(0),root.command(1),root2.command(0)].iter().map(|id|engine.world().command_ledger.get(*id).unwrap()).collect();
    assert!(receipts.iter().all(|r|r.outcome.state==cathedral_sim::receipts::ReceiptState::Accepted));
    let witnesses=json!({"boundary_seconds":boundary.seconds(),"partner":partner,"peer":peer,"player_position_m":at.to_array(),"utterance":utterance,"reply":reply,"draft_text":draft_text,"submitted_inputs":submitted_inputs,"recording_receipts":receipts,"tts_requests":voice_inputs,"stt_calls":*transcription,"submitted_prompts":service.prompts,"provider_submissions":service.prompts.len(),"first_messages":relevant(&first),"committed_messages":relevant(&committed),"parked_messages":relevant(&parked_output),"batch_messages":relevant(&batch_output),"second_batch_messages":relevant(&second_batch),"all_speech_messages":driver.all_speech_messages,"poll_count":driver.polls,"maximum_poll_step_seconds":driver.maximum_poll_step_seconds,"coarse_discard_diagnostics":driver.coarse_discard_diagnostics,"all_message_count":driver.messages,"all_message_debug_bytes":driver.digest.bytes,"all_message_digest_algorithm":"fnv1a64-debug-stream-v1","all_message_digest":format!("{:016x}",driver.digest.hash)});
    let mut phases=Phases::default();let mut metadata=None;
    for _ in 0..args.samples{
        let budget=CheckpointBudget::default();let initial=||budget.reserve(Cohort::SavePayload,4096).unwrap();
        let preflight=time(&mut phases.preflight_us,||engine.checkpoint_speech_cost(boundary,initial()))?;let cost=*preflight.value();drop(preflight);
        let dto=time(&mut phases.export_us,||engine.export_speech_checkpoint(boundary,initial()))?;let counts=dto.value().counts(context);
        assert_eq!(counts.characters,match args.mode{Mode::Authored=>520,Mode::Populated=>2520});
        assert_eq!(counts.accepted_recordings,3);assert_eq!(counts.parked,1);assert_eq!(counts.batch_pending,2);assert_eq!(counts.streams,2);assert_eq!(counts.captures,3);assert_eq!(counts.available_texts,1);assert_eq!(counts.available_text_bytes,draft_text.len());assert_eq!(counts.semantic_receipts,3);assert_eq!(counts.unique_roots,2);
        let bytes=time(&mut phases.encode_us,||dto.encode())?;let wire:serde_json::Value=serde_json::from_slice(bytes.value())?;
        let input=budget.reserve(Cohort::LoadCandidate,bytes.value().len()+4096)?;
        let decoded=time(&mut phases.decode_validate_us,||EngineSpeechDtoV1::decode(bytes.value(),input,context))?;
        let candidate=time(&mut phases.candidate_validate_us,||decoded.into_candidate(context))?;
        assert_eq!(candidate.value().counts(context),counts);assert_eq!(candidate.value().streams()[0].available_text(),Some(draft_text));
        let current=json!({"scenario":"speech-ordinary-interrupted-inputs-v1","counts":counts,"witnesses":witnesses,"boundary_speech":wire,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
        if let Some(prior)=&metadata{assert_eq!(prior,&current);}else{metadata=Some(current);}
        time(&mut phases.drop_us,||{drop(candidate);drop(bytes);});assert_eq!(budget.retained_bytes(),0);
    }
    let mut output=metadata.unwrap();output["mode"]=json!(match args.mode{Mode::Authored=>"authored",Mode::Populated=>"populated"});output["samples"]=json!(args.samples);output["placement"]=placement;
    for(k,v)in serde_json::to_value(phases)?.as_object().unwrap(){output[k]=v.clone();}
    fs::write(args.output,serde_json::to_vec_pretty(&output)?)?;Ok(())
}
#[derive(Default,Serialize)]
struct SttCalls{available:usize,batch:Vec<(u64,PathBuf,cathedral_sim::SttBackendKind)>,realtime:Vec<String>,inspected:Vec<PathBuf>,discarded:Vec<PathBuf>}
struct Stt(Arc<std::sync::Mutex<SttCalls>>);
impl cathedral_sim::Transcription for Stt{
    fn available(&self,_:cathedral_sim::SttBackendKind)->bool{self.0.lock().unwrap().available+=1;true}
    fn submit_batch(&mut self,job:cathedral_sim::TranscriptionJobId,path:PathBuf,kind:cathedral_sim::SttBackendKind)->Result<(),cathedral_sim::SttSubmitError>{self.0.lock().unwrap().batch.push((job.0,path,kind));Ok(())}
    fn realtime_begin(&mut self,key:&str)->bool{self.0.lock().unwrap().realtime.push(format!("begin:{key}"));true}
    fn realtime_append(&mut self,key:&str,samples:&[i16])->bool{self.0.lock().unwrap().realtime.push(format!("append:{key}:{}",samples.len()));true}
    fn realtime_commit(&mut self,key:&str)->bool{self.0.lock().unwrap().realtime.push(format!("commit:{key}"));true}
    fn realtime_clear(&mut self,key:&str){self.0.lock().unwrap().realtime.push(format!("clear:{key}"));}
    fn recording_seconds(&self,path:&std::path::Path)->Option<f64>{self.0.lock().unwrap().inspected.push(path.to_owned());Some(0.01)}
    fn discard_recording(&mut self,path:&std::path::Path){self.0.lock().unwrap().discarded.push(path.to_owned());}
}
'''
s+=p.read_text()[p.read_text().index('struct Voices('):]
Path('crates/cathedral-backends/examples/alibi_speech_cost.rs').write_text(s)
