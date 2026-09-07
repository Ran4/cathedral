//! Live prompt experiments using the same provider client and token caps as the game.
//! Run from the workspace root; no renderer, speech workers or fake cognition.
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use cathedral_backends::{BackendRuntime, BackendsConfig, BackendsOptions, Environment, LlmClient};
use clap::Parser;
use futures_util::{StreamExt, stream};
use serde::Deserialize;
use serde_json::json;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    requests: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "gpt-5.6-luna")]
    model: String,
    #[arg(long, default_value = "openai")]
    provider: String,
    #[arg(long, default_value_t = 3)]
    concurrency: usize,
}

#[derive(Deserialize)]
struct Trial {
    name: String,
    prompt: String,
    #[serde(default = "budget")]
    max_output_tokens: u32,
}

fn budget() -> u32 {
    350
}

fn prompt_path(prompt: &str) -> PathBuf {
    let mut hash = DefaultHasher::new();
    prompt.hash(&mut hash);
    PathBuf::from(format!("prompts/{:016x}.txt", hash.finish()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let trials: Vec<Trial> = serde_json::from_str(&fs::read_to_string(&args.requests)?)?;
    for trial in &trials {
        if trial.name.is_empty()
            || !trial
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("trial names must be nonempty filename-safe identifiers".into());
        }
    }
    fs::create_dir_all(&args.output)?;
    fs::create_dir_all(args.output.join("prompts"))?;
    for trial in &trials {
        let path = args.output.join(prompt_path(&trial.prompt));
        if path.exists() && fs::read_to_string(&path)? != trial.prompt {
            return Err("prompt archive key collision".into());
        }
        fs::write(path, &trial.prompt)?;
    }
    let options = BackendsOptions::default();
    let mut environment = Environment::from_process(options.dotenv_path.as_deref());
    environment.set("LLM_MODEL", args.model);
    environment.set("LLM_PROVIDER", args.provider);
    let config = BackendsConfig::resolve(&environment, &options);
    let client = Arc::new(LlmClient::new(config.llm?)?);
    let runtime = BackendRuntime::new()?;
    let start = Instant::now();
    let failures = runtime.block_on(async {
        stream::iter(trials)
            .map(|trial| {
                let client = client.clone();
                let output = &args.output;
                async move {
                    let start = Instant::now();
                    let result = client
                        .complete_with_budget(trial.prompt.clone(), Some(trial.max_output_tokens))
                        .await;
                    let elapsed = start.elapsed().as_secs_f64();
                    let failed = result.is_err();
                    let row = json!({
                        "name": trial.name,
                        "model": client.settings().model,
                        "prompt_path": prompt_path(&trial.prompt),
                        "max_output_tokens": trial.max_output_tokens,
                        "duration_seconds": elapsed,
                        "answer": result.as_ref().ok(),
                        "error": result.as_ref().err().map(ToString::to_string),
                    });
                    fs::write(
                        output.join(format!("{}.json", trial.name)),
                        serde_json::to_string_pretty(&row).unwrap(),
                    )
                    .expect("write trial evidence");
                    println!(
                        "{}: {} ({elapsed:.2}s)",
                        trial.name,
                        if failed { "ERROR" } else { "ok" }
                    );
                    usize::from(failed)
                }
            })
            .buffer_unordered(args.concurrency.clamp(1, 8))
            .fold(0, |sum, failures| async move { sum + failures })
            .await
    });
    let usage = client.usage();
    let tokens: Vec<_> = usage
        .per_model()
        .iter()
        .map(|(model, u)| {
            json!({
                "model": model,
                "prompt_tokens": u.prompt_tokens,
                "cached_prompt_tokens": u.cached_prompt_tokens,
                "completion_tokens": u.completion_tokens,
            })
        })
        .collect();
    fs::write(
        args.output.join("summary.json"),
        serde_json::to_string_pretty(&json!({
            "failures": failures,
            "duration_seconds": start.elapsed().as_secs_f64(),
            "usage": tokens,
            "cost_usd": usage.run_cost_usd(),
        }))?,
    )?;
    if failures > 0 {
        return Err(format!("{failures} provider calls failed; inspect recorded errors").into());
    }
    Ok(())
}
