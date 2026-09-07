//! Multi-turn live conversation experiments through Engine::poll, without Bevy.
use std::{cell::RefCell, collections::VecDeque, fs, path::PathBuf, rc::Rc, time::Instant};

use cathedral_backends::{BackendRuntime, BackendsConfig, BackendsOptions, Environment, LlmClient};
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Completion, Engine, EngineCommand,
    EngineConfig, EngineMessage, IdleCognitionMode, NullSight, NullTranscription, NullTts,
    PromptEnv, RequestId, SoundCatalog, Vec3, WorldSeed, apply_action,
};
use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    scene: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "gpt-5.6-luna")]
    model: String,
}

#[derive(Deserialize)]
struct Scene {
    seed: Value,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    label: String,
    text: Option<String>,
    focus: Option<String>,
    #[serde(default)]
    focus_seconds: f64,
    #[serde(default)]
    silence_seconds: f64,
    #[serde(default)]
    moves: Vec<(String, [f64; 3])>,
    #[serde(default)]
    percepts: Vec<(String, String)>,
    #[serde(default)]
    npc_lines: Vec<(String, String)>,
    #[serde(default = "turns")]
    turns: usize,
}

fn turns() -> usize {
    3
}

type Request = (RequestId, String, Option<u32>);
#[derive(Clone, Default)]
struct Capture(Rc<RefCell<(u64, VecDeque<Request>)>>);
impl Cognition for Capture {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.request_with_budget(prompt, None)
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        cap: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        let mut state = self.0.borrow_mut();
        state.0 += 1;
        let id = RequestId(state.0);
        state.1.push_back((id, prompt, cap));
        Ok(id)
    }
}

fn collect(engine: &mut Engine, now: f64, commands: Vec<EngineCommand>, events: &mut Vec<Value>) {
    for message in engine.poll(now, commands) {
        match message {
            EngineMessage::Speech {
                speaker_id,
                target_id,
                text,
                recipient_ids,
                ..
            } => {
                println!("{now:6.2} {speaker_id}: {text}");
                events.push(json!({"at": now, "speaker": speaker_id, "target": target_id, "text": text, "hearers": recipient_ids}));
            }
            EngineMessage::Diagnostic(line) => {
                events.push(json!({"at": now, "diagnostic": line}));
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let scene: Scene = serde_json::from_str(&fs::read_to_string(&args.scene)?)?;
    let seed = WorldSeed::from_json_str(&scene.seed.to_string())?;
    let env = PromptEnv::new(
        &fs::read_to_string("assets/prompts/turn.j2")?,
        &fs::read_to_string("assets/prompts/night.j2")?,
        &fs::read_to_string("assets/prompts/strings.toml")?,
    )?;
    let options = BackendsOptions::default();
    let mut environment = Environment::from_process(options.dotenv_path.as_deref());
    environment.set("LLM_MODEL", args.model);
    environment.set("LLM_PROVIDER", "openai".to_string());
    let config = BackendsConfig::resolve(&environment, &options);
    let client = LlmClient::new(config.llm?)?;
    let runtime = BackendRuntime::new()?;
    let capture = Capture::default();
    let player = ActorId::from_raw("player");
    let origin = seed.character(&player).ok_or("player missing")?.position_m;
    let mut engine_config = EngineConfig {
        idle_mode: IdleCognitionMode::Stage,
        idle_requires_news: true,
        ..EngineConfig::default()
    };
    engine_config.idle_curiosity.enabled = true;
    engine_config.idle_curiosity.scale = 0.0;
    engine_config.clock =
        cathedral_sim::WorldClock::new(86_400.0, cathedral_sim::Office::Dayspring, 2, 0.05);
    let mut engine = Engine::new(
        engine_config,
        &seed,
        AreaMap::default(),
        SoundCatalog::from_toml_str(&fs::read_to_string("assets/sounds/catalog.toml")?)?,
        env,
        Box::new(capture.clone()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities {
            llm: true,
            ..Capabilities::default()
        },
        (origin, 0.0),
        0,
        0.0,
    )?;
    fs::create_dir_all(&args.output)?;
    let mut now = 0.0;
    let mut sequence = 0;
    let mut events = Vec::new();
    let mut steps = Vec::new();
    let mut exchanges = 0;
    for step in scene.steps {
        println!("STEP {}", step.label);
        now += step.silence_seconds;
        for (who, [x, y, z]) in step.moves {
            engine
                .world_mut()
                .characters
                .get_mut(&ActorId::from_raw(who))
                .ok_or("move actor missing")?
                .state
                .position_m = Vec3::new(x, y, z);
        }
        for (who, text) in step.percepts {
            engine
                .world_mut()
                .characters
                .get_mut(&ActorId::from_raw(who))
                .ok_or("percept actor missing")?
                .notify_percept(text);
        }
        for (who, text) in step.npc_lines {
            apply_action(
                engine.world_mut(),
                &ActorId::from_raw(who),
                "say",
                &json!({"target": "player", "text": text}),
            )?;
        }
        collect(
            &mut engine,
            now,
            vec![EngineCommand::PlayerAttention {
                actor_id: step.focus.map(ActorId::from_raw),
            }],
            &mut events,
        );
        now += step.focus_seconds;
        if let Some(text) = step.text {
            sequence += 1;
            let position_m = engine.world().characters[&player].position_m();
            collect(
                &mut engine,
                now,
                vec![EngineCommand::PlayerSay {
                    request_id: format!("line-{sequence}"),
                    text,
                    position_m,
                    spatial_seq: sequence,
                }],
                &mut events,
            );
        }
        let mut done = 0;
        let mut idle_since = now;
        for _ in 0..2000 {
            let request = capture.0.borrow_mut().1.pop_front();
            if let Some((id, prompt, cap)) = request {
                let actor = engine.scheduler().in_flight_actor_id().cloned();
                let start = Instant::now();
                let result = runtime.block_on(client.complete_with_budget(prompt.clone(), cap));
                let duration = start.elapsed().as_secs_f64();
                exchanges += 1;
                fs::write(
                    args.output.join(format!("turn-{exchanges:03}.json")),
                    serde_json::to_string_pretty(&json!({
                        "step": step.label, "actor": actor, "model": client.settings().model, "prompt": prompt,
                        "answer": result.as_ref().ok(), "error": result.as_ref().err().map(ToString::to_string),
                        "duration_seconds": duration, "max_output_tokens": cap,
                    }))?,
                )?;
                let answer = result?;
                done += 1;
                now += duration;
                collect(
                    &mut engine,
                    now,
                    vec![EngineCommand::LlmCompletion(Completion {
                        request_id: id,
                        result: Ok(answer),
                        duration_seconds: duration,
                    })],
                    &mut events,
                );
                idle_since = now;
            }
            if engine.scheduler().in_flight_actor_id().is_none()
                && (done >= step.turns || now - idle_since > 12.0)
            {
                break;
            }
            now += 0.1;
            collect(&mut engine, now, vec![], &mut events);
        }
        if engine.scheduler().in_flight_actor_id().is_some() {
            return Err("simulation step exhausted its poll bound with work in flight".into());
        }
        steps.push(json!({"label": step.label, "at": now, "calls": done,
            "partner": engine.conversation_partner(now)}));
        fs::write(
            args.output.join("transcript.json"),
            serde_json::to_string_pretty(&json!({"events": events, "steps": steps}))?,
        )?;
    }
    let usage: Vec<_> = client.usage().per_model().iter().map(|(m,u)| json!({
        "model": m, "prompt_tokens": u.prompt_tokens, "cached_prompt_tokens": u.cached_prompt_tokens,
        "completion_tokens": u.completion_tokens,
    })).collect();
    fs::write(
        args.output.join("usage.json"),
        serde_json::to_string_pretty(
            &json!({"usage": usage, "calls": exchanges, "cost_usd": client.run_cost_usd()}),
        )?,
    )?;
    Ok(())
}
