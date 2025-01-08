use clap::Parser;
use dotenv::dotenv;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use std::path::PathBuf;
use tracing::info;
use zkemail_core::EmailWithRegexVerifierOutput;
use zkemail_helpers::{generate_email_inputs, generate_email_with_regex_inputs};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const EMAIL_VERIFY_ELF: &[u8] = include_elf!("email_verify");
pub const EMAIL_WITH_REGEX_ELF: &[u8] = include_elf!("email_with_regex_verify");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    execute: bool,

    #[clap(long)]
    prove: bool,

    #[clap(long)]
    from_domain: String,

    #[clap(long)]
    email_path: PathBuf,

    #[clap(long)]
    regex_config: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    if args.from_domain.is_empty() {
        eprintln!("Error: You must specify a from domain");
        std::process::exit(1);
    }

    if args.email_path.as_os_str().is_empty() {
        eprintln!("Error: You must specify an email path");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    // Get the right ELF
    let image = if args.regex_config.is_some() {
        EMAIL_WITH_REGEX_ELF
    } else {
        EMAIL_VERIFY_ELF
    };

    // Generate appropriate input
    if let Some(regex_config) = args.regex_config {
        let input =
            generate_email_with_regex_inputs(&args.from_domain, &args.email_path, &regex_config)
                .await
                .expect("Failed to generate email with regex inputs");
        stdin.write(&input);
    } else {
        let input = generate_email_inputs(&args.from_domain, &args.email_path)
            .await
            .expect("Failed to generate email inputs");
        stdin.write(&input);
    }

    if args.execute {
        // Execute the program
        let (mut output, report) = client.execute(image, &stdin).run().unwrap();
        info!("Program executed successfully.");

        let output = output.read::<EmailWithRegexVerifierOutput>();
        info!("Output: {:?}", output);

        info!("Number of cycles: {}", report.total_instruction_count());
    } else {
        // NOTE: Does not work with prover network.
        // Setup the program for proving.
        let (pk, vk) = client.setup(image);

        // Generate the proof
        let start = std::time::Instant::now();
        let proof = client
            .prove(&pk, &stdin)
            .groth16()
            .run()
            .expect("failed to generate proof");
        let duration = start.elapsed().as_secs_f64();

        info!("Successfully generated proof in {:.2}s!", duration);
        info!("Proof: {:?}", proof);

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        info!("Successfully verified proof!");
    }
}
