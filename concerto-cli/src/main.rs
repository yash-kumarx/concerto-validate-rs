//! CLI for poking the validator without writing another app.
//!
//! I used this a lot while building the POC. Faster to run one command on a
//! fixture than wire up a new demo every time I wanted to sanity-check a case.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use concerto_core::declaration::{ClassDeclaration, Declaration};
use concerto_core::ModelManager;

/// Command-line entry point.
#[derive(Parser, Debug)]
#[command(
    name = "concerto-rs",
    version = env!("CARGO_PKG_VERSION"),
    about = "Concerto validator POC"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Validates one JSON instance against one type.
    Validate {
        /// Model JSON file.
        #[arg(short, long, value_name = "FILE")]
        model: PathBuf,

        /// Instance JSON file.
        #[arg(short, long, value_name = "FILE")]
        instance: PathBuf,

        /// Requested type name.
        #[arg(short = 't', long, value_name = "FQN")]
        r#type: String,

        /// Print raw JSON instead of plain text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Checks that a model JSON file loads.
    Check {
        /// Model JSON file.
        #[arg(short, long, value_name = "FILE")]
        model: PathBuf,
    },

    /// Prints declarations and properties from a loaded model.
    Info {
        /// Model JSON file.
        #[arg(short, long, value_name = "FILE")]
        model: PathBuf,
    },

    /// Runs a quick runtime benchmark.
    Bench {
        /// Model JSON file.
        #[arg(short, long, value_name = "FILE")]
        model: PathBuf,

        /// Instance JSON file.
        #[arg(short, long, value_name = "FILE")]
        instance: PathBuf,

        /// Requested type name.
        #[arg(short = 't', long, value_name = "FQN")]
        r#type: String,

        /// Iteration count.
        #[arg(short = 'n', long, default_value_t = 10_000)]
        iterations: u32,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Validate {
            model,
            instance,
            r#type,
            json,
        } => run_validate(&model, &instance, &r#type, json),
        Command::Check { model } => run_check(&model),
        Command::Info { model } => run_info(&model),
        Command::Bench {
            model,
            instance,
            r#type,
            iterations,
        } => run_bench(&model, &instance, &r#type, iterations),
    };

    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn read_file(path: &PathBuf, label: &str) -> Result<String, Box<dyn std::error::Error>> {
    fs::read_to_string(path)
        .map_err(|error| format!("can't read {label} file '{}': {error}", path.display()).into())
}

fn load_model(path: &PathBuf) -> Result<ModelManager, Box<dyn std::error::Error>> {
    let json = read_file(path, "model")?;
    let mut mm = ModelManager::new();
    mm.add_model_from_json(&json)
        .map_err(|error| format!("model load failed: {error}"))?;
    Ok(mm)
}

fn load_instance(path: &PathBuf) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let text = read_file(path, "instance")?;
    serde_json::from_str(&text).map_err(|error| format!("bad instance json: {error}").into())
}

fn run_validate(
    model_path: &PathBuf,
    instance_path: &PathBuf,
    type_name: &str,
    as_json: bool,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mm = load_model(model_path)?;
    let instance = load_instance(instance_path)?;
    let result = mm
        .validate_instance(&instance, type_name)
        .map_err(|error| format!("validation failed: {error}"))?;

    if as_json {
        let errors = result
            .errors
            .iter()
            .map(|error| serde_json::json!({"path": error.path, "message": error.message}))
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::json!({"valid": result.valid, "errors": errors})
        );
    } else if result.valid {
        println!("VALID");
    } else {
        println!("INVALID");
        for error in &result.errors {
            println!("{}: {}", error.path, error.message);
        }
    }

    Ok(if result.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_check(model_path: &PathBuf) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mm = load_model(model_path)?;
    let namespaces = sorted_namespaces(&mm);

    if namespaces.is_empty() {
        println!("namespace: <none>");
    } else {
        for namespace in namespaces {
            println!("namespace: {namespace}");
        }
    }

    println!("declarations: {}", mm.all_declarations().count());
    Ok(ExitCode::SUCCESS)
}

fn run_info(model_path: &PathBuf) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mm = load_model(model_path)?;
    let mut decls = mm
        .all_declarations()
        .map(|decl| (decl.namespace().to_string(), decl.name().to_string()))
        .collect::<Vec<_>>();
    decls.sort();

    for (namespace, name) in decls {
        let decl = mm.resolve_type(&format!("{namespace}.{name}"))?;
        match decl {
            Declaration::Concept(class_decl) => print_class_like("concept", class_decl),
            Declaration::Asset(class_decl) => print_class_like("asset", class_decl),
            Declaration::Participant(class_decl) => print_class_like("participant", class_decl),
            Declaration::Transaction(class_decl) => print_class_like("transaction", class_decl),
            Declaration::Event(class_decl) => print_class_like("event", class_decl),
            Declaration::Enum(enum_decl) => {
                println!(
                    "enum {} ({} values)",
                    enum_decl.name,
                    enum_decl.values.len()
                );
                for value in &enum_decl.values {
                    println!("  {value}: EnumValue");
                }
            }
            Declaration::Scalar(scalar_decl) => {
                println!("scalar {} (0 properties)", scalar_decl.name);
                println!("  type: {}", scalar_decl.scalar_type.describe());
            }
            Declaration::Map(map_decl) => {
                println!("map {} (0 properties)", map_decl.name);
                println!("  key: {}", map_decl.key_type.describe());
                println!("  value: {}", map_decl.value_type.describe());
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn run_bench(
    model_path: &PathBuf,
    instance_path: &PathBuf,
    type_name: &str,
    iterations: u32,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if iterations == 0 {
        return Err("iterations must be > 0".into());
    }

    let mm = load_model(model_path)?;
    let instance = load_instance(instance_path)?;

    let mut min_us = f64::MAX;
    let mut max_us = 0.0_f64;
    let mut total_us = 0.0_f64;

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = mm
            .validate_instance(&instance, type_name)
            .map_err(|error| format!("validation failed during bench: {error}"))?;
        let elapsed_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        total_us += elapsed_us;
        min_us = min_us.min(elapsed_us);
        max_us = max_us.max(elapsed_us);
    }

    let mean_us = total_us / f64::from(iterations);

    println!("iterations: {iterations}");
    println!("mean_us: {mean_us:.3}");
    println!("min_us: {min_us:.3}");
    println!("max_us: {max_us:.3}");

    Ok(ExitCode::SUCCESS)
}

fn sorted_namespaces(mm: &ModelManager) -> Vec<&str> {
    let mut namespaces = mm.namespaces().collect::<Vec<_>>();
    namespaces.sort();
    namespaces
}

fn print_class_like(kind: &str, class_decl: &ClassDeclaration) {
    let mut props = class_decl.properties.iter().collect::<Vec<_>>();
    props.sort_by(|(left_name, _), (right_name, _)| left_name.cmp(right_name));

    println!("{kind} {} ({} properties)", class_decl.name, props.len());

    for (name, property) in props {
        let mut type_label = property.property_type.describe();
        if property.is_array {
            type_label = format!("Array<{type_label}>");
        }
        if property.is_optional {
            type_label = format!("{type_label} optional");
        }
        println!("  {name}: {type_label}");
    }
}
