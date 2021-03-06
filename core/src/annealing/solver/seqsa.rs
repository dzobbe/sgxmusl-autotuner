/// ///////////////////////////////////////////////////////////////////////////
///  File: Annealing/Solver/SS.rs
/// ///////////////////////////////////////////////////////////////////////////
///  Copyright 2017 Giovanni Mazzeo
///
///  Licensed under the Apache License, Version 2.0 (the "License");
///  you may not use this file except in compliance with the License.
///  You may obtain a copy of the License at
///
///      http://www.apache.org/licenses/LICENSE-2.0
///
///  Unless required by applicable law or agreed to in writing, software
///  distributed under the License is distributed on an "AS IS" BASIS,
///  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
///  See the License for the specific language governing permissions and
///  limitations under the License.
/// ///////////////////////////////////////////////////////////////////////////

/// ****************************************************************************
/// *****************************************************************************
/// **
/// SEQuential Simulated Annealing (SEQSA)
/// *
/// *****************************************************************************
/// ****************************************************************************
use annealing::solver::Solver;
use annealing::problem::Problem;
use annealing::cooler::{Cooler, StepsCooler, TimeCooler};
use annealing::solver::common::MrResult;


use res_emitters;
use res_emitters::Emitter;

use shared::TunerParameter;

use time;
use CoolingSchedule;
use EnergyType;
use pbr;
use rand;
use libc;
use num_cpus;

use rand::{Rng, thread_rng};
use rand::distributions::{Range, IndependentSample};
use ansi_term::Colour::Green;
use std::collections::HashMap;
use pbr::{ProgressBar, MultiBar};


#[derive(Debug, Clone)]
pub struct Seqsa {
    pub tuner_params: TunerParameter,
    pub res_emitter: Emitter,
}

impl Solver for Seqsa {
    fn solve(&mut self, problem: &mut Problem, num_workers: usize) -> MrResult {


        let cooler = StepsCooler {
            max_steps: self.tuner_params.max_step,
            min_temp: self.tuner_params.min_temp.unwrap(),
            max_temp: self.tuner_params.max_temp.unwrap(),
        };

        let mut rng = thread_rng();
        let range = Range::new(0.0, 1.0);

        println!("{}",Green.paint("\n-------------------------------------------------------------------------------------------------------------------"));
        println!(
            "{} Initialization Phase: Evaluation of Energy for Default Parameters",
            Green.paint("[TUNER]")
        );
        println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));

        let mut start_time = time::precise_time_ns();

        let mut state = problem.initial_state();
        let mut energy = match problem.energy(&state, 0) {
            Some(nrg) => nrg,
            None => panic!("The initial configuration does not allow to calculate the energy"),
        };



        let mut exec_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
        let mut elapsed_time = 0.0;
        let mut temperature: f64 = self.tuner_params.max_temp.unwrap();
        let mut accepted = 0;
        let mut subsequent_rejected = 0;
        let mut last_nrg = energy;


        start_time = time::precise_time_ns();


        let cpu_time = 0.0;
        for elapsed_steps in 0..self.tuner_params.max_step {

            elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;


            let time_2_complete_mins =
                exec_time * ((self.tuner_params.max_step - elapsed_steps) as f64) / 60.0;
            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));
            println!(
                "{} Completed Steps: {:.2} - Percentage of Completion: {:.2}% - Estimated \
                      time to Complete: {:.2} Mins",
                Green.paint("[TUNER]"),
                elapsed_steps,
                (elapsed_steps as f64 / cooler.max_steps as f64) * 100.0,
                time_2_complete_mins as usize
            );
            println!(
                "{} Total Accepted Solutions: {:?} - Subsequent Rejected: {:?} - Current \
                      Temperature: {:.2} - Elapsed Time: {:.2} s",
                Green.paint("[TUNER]"),
                accepted,
                subsequent_rejected,
                temperature,
                elapsed_time
            );
            println!("{} Accepted State: {:?}", Green.paint("[TUNER]"), state);
            println!(
                "{} Accepted Energy: {:.4} - Last Measured Energy: {:.4}",
                Green.paint("[TUNER]"),
                energy,
                last_nrg
            );

            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));


            state = {

                let next_state =
                    problem.new_state(&state, self.tuner_params.max_step, elapsed_steps);

                let accepted_state = match problem.energy(&next_state, 0) {
                    Some(new_energy) => {
                        last_nrg = new_energy;

                        let de = match self.tuner_params.energy {
                            EnergyType::throughput => new_energy - energy,
                            EnergyType::latency => -(new_energy - energy), 
                        };

                        if subsequent_rejected > 400 {
                            println!("{} Convergence Reached!!!", Green.paint("[TUNER]"));
                            break;
                        }

                        if de > 0.0 || range.ind_sample(&mut rng) <= (de / temperature).exp() {
                            accepted += 1;
                            energy = new_energy;

                            if de > 0.0 {
                                subsequent_rejected = 0;
                            }

                            self.res_emitter.send_update(
                                temperature,
                                elapsed_time,
                                cpu_time,
                                new_energy,
                                &next_state,
                                energy,
                                &next_state,
                                elapsed_steps,
                                0,
                            );
                            next_state

                        } else {
                            subsequent_rejected += 1;
                            self.res_emitter.send_update(
                                temperature,
                                elapsed_time,
                                cpu_time,
                                new_energy,
                                &next_state,
                                energy,
                                &state,
                                elapsed_steps,
                                0,
                            );
                            state
                        }


                    }
                    None => {
                        println!(
                            "{} The current configuration parameters cannot be evaluated. \
                                  Skip!",
                            Green.paint("[TUNER]")
                        );
                        state
                    }
                };

                accepted_state
            };


            temperature = match self.tuner_params.cooling {
                CoolingSchedule::linear => cooler.linear_cooling(elapsed_steps),
                CoolingSchedule::exponential => cooler.exponential_cooling(elapsed_steps),
                CoolingSchedule::basic_exp_cooling => cooler.basic_exp_cooling(temperature),
            };
        }

        MrResult {
            energy: energy,
            state: state,
        }

    }
}
