extern crate clap;
extern crate blstrs;
use clap::{command, Parser};
use groth16::api::*;
use groth16::groth16::*;
use starky::prove::stark_prove;

use std::fs::File;
use std::io::Write;

///Gpu 
use nvml_wrapper::Nvml;
use algebraic::witness::{load_input_for_witness, WitnessCalculator};
use algebraic_gpu::circom_circuit::CircomCircuit;
use algebraic_gpu::reader;
use blstrs::{Bls12, Scalar};
use ff::{Field, PrimeField};
use num_traits::Zero;
//use rand_new::rngs::OsRng;
use rand::rngs::OsRng;



#[derive(Debug, Parser, Default)]
#[command(about, version, no_binary_name(true))]
struct Cli {
    #[arg(short, long = "input_file", default_value = "multiplier.input.json")]
    input_file: String,
    #[arg(short, long = "circuit_file_bls12", default_value = "mycircuit_bls12381.r1cs")]
    circuit_file_bls12: String,
    #[arg(short, long = "wasm_file_bls12", default_value = "mycircuit_bls12381.wasm")]
    wasm_file_bls12: String,
   
}

//use gevulot_common::WORKSPACE_PATH;
use gevulot_shim::{Task, TaskResult};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    gevulot_shim::run(run_task)
}

fn run_task(task: Task) -> Result<TaskResult> {
    env_logger::init();

    println!("0xEigenLabs gpu prover : task.args: {:?}", &task.args);

    let args = Cli::parse_from(&task.args);

    log::info!(
        "parameters: input_file:{} ; circuit_file_bls12:{} ; wasm_file_bls12:{}",
        args.input_file,
        args.circuit_file_bls12,
        args.wasm_file_bls12
    );

    let mut log_file = File::create("/workspace/debug.log")?;
    write!(log_file, "input_file:{}\n", &args.input_file)?;
    write!(log_file, "circuit_file_bls12:{}\n", &args.circuit_file_bls12)?;
    write!(log_file, "wasm_file_bls12:{}\n", &args.wasm_file_bls12)?;
    
/*
    let exec_result = groth16_proof_gevulot(&args.input_file , &args.circuit_file_bls12, &args.wasm_file_bls12);

    match exec_result {
        Err(x) => {
            log::info!("The prover has error: {}", x);
            write!(log_file, "The prover has error: {}\n", x)?;
        }
        _ => write!(log_file, "The prover executes successfully.\n")?,
    };
*/
        let nvml = Nvml::init()?;

    // 获取 GPU 设备的数量
    let device_count = nvml.device_count().unwrap();
    log::info!("Number of GPUs: {}", device_count);
    write!(log_file, "Number of GPUs: {}\n", device_count);
    // 遍历所有 GPU 设备
    for i in 0..device_count {
        let handle = nvml.device_get_handle_by_index(i as u32).unwrap();
        let info = nvml.device_get_info(handle);

        log::info!("GPU {}:", i);
        log::info!("  Name: {}", info.name.unwrap());
        write!(log_file, "name: {}\n", info.name.unwrap());
    }


    log::info!("The prover executes successfully");

    // Write generated proof to a file.
    //std::fs::write("/workspace/proof.json", b"this is a proof a.")?;

    //return three files for Verifier
    task.result(
        vec![],
        vec![
            //String::from("/workspace/proof.json"),
           // String::from("/workspace/stark_verfier.circom"),
            String::from("/workspace/debug.log"),
        ],
    )
}

fn groth16_proof_gevulot(input_file: &String, circuit_file_bls12: &String, wasm_file_bls12: &String) -> Result<()> {
    //1. SRS
    let t = std::time::Instant::now();
    let circuit: CircomCircuit<Scalar> = CircomCircuit {
        r1cs: reader::load_r1cs(circuit_file_bls12),
        witness: None,
        wire_mapping: None,
        aux_offset: 0,
    };
    let params = Groth16::circuit_specific_setup(circuit, &mut OsRng)?;
    let elapsed = t.elapsed().as_secs_f64();
    println!("1-groth16-bls12381 setup run time: {} secs", elapsed);

    //2. Prove
    let t1 = std::time::Instant::now();
    let mut wtns = WitnessCalculator::from_file(wasm_file_bls12)?;
    let inputs = load_input_for_witness(input_file);
    let w = wtns.calculate_witness(inputs, false).unwrap();
    let w = w
        .iter()
        .map(|wi| {
            if wi.is_zero() {
                <Bls12 as Engine>::Fr::ZERO
            } else {
                // println!("wi: {}", wi);
                <Bls12 as Engine>::Fr::from_str_vartime(&wi.to_string()).unwrap()
            }
        })
        .collect::<Vec<_>>();
    let circuit1: CircomCircuit<Scalar> = CircomCircuit {
        r1cs: reader::load_r1cs(circuit_file_bls12),
        witness: Some(w),
        wire_mapping: None,
        aux_offset: 0,
    };
    let inputs = circuit1.get_public_inputs().unwrap();
    let proof: bellperson::groth16::Proof<Bls12> =
        Groth16::prove(&params.0, circuit1, &mut OsRng)?;
    let elapsed1 = t1.elapsed().as_secs_f64();
    println!("2-groth16-bls12381 prove run time: {} secs", elapsed1);

    //3. Verify
    let t2 = std::time::Instant::now();
    let verified = Groth16::<_, CircomCircuit<Scalar>>::verify_with_processed_vk(
        &params.1, &inputs, &proof,
    )?;
    let elapsed2 = t2.elapsed().as_secs_f64();
    println!("3-groth16-bls12381 verify run time: {} secs", elapsed2);

    assert!(verified);

    Ok(())
}
