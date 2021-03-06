/// ///////////////////////////////////////////////////////////////////////////
///  File: Annealing/Solver/MIPS.rs
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
/// Multiple Independent Run (MIR)
/// *
/// *****************************************************************************
/// ****************************************************************************
use annealing::solver::Solver;
use annealing::problem::Problem;
use annealing::cooler::{Cooler, StepsCooler, TimeCooler};
use annealing::solver::common;
use annealing::solver::common::MrResult;
use res_emitters;
use res_emitters::Emitter;
use annealing::solver::common::IntermediateResults;

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
use std::thread;
use std::sync::mpsc::channel;


#[derive(Debug, Clone)]
pub struct Mir {
    pub tuner_params: TunerParameter,
    pub res_emitter: Emitter,
}

impl Solver for Mir {
    fn solve(&mut self, problem: &mut Problem, num_workers: usize) -> MrResult {

        let cooler = StepsCooler {
            max_steps: self.tuner_params.max_step,
            min_temp: self.tuner_params.min_temp.unwrap(),
            max_temp: self.tuner_params.max_temp.unwrap(),
        };


        ("{}",Green.paint("\n-------------------------------------------------------------------------------------------------------------------"));
        println!(
            "{} Initialization Phase: Evaluation of Energy for Default Parameters",
            Green.paint("[TUNER]")
        );
        println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));


        let mut elapsed_steps = common::SharedGenericCounter::new();

        // Creation of the pool of Initial States. It will be composed by the initial default state
        // given by the user and by other num_workers-1 states generated in a random way
        let mut initial_state = problem.initial_state();
        let mut initial_states_pool = common::StatesPool::new();
        initial_states_pool.push(initial_state.clone());
        for i in 1..num_workers {
            initial_states_pool.push(problem.rand_state());
        }

        // Create a muti-bar
        let mut mb = MultiBar::new();

        let threads_res = common::ThreadsResults::new();

        let mut overall_start_time = time::precise_time_ns();


        let handles: Vec<_> = (0..num_workers).map(|worker_nr| {
 				
				let mut pb=mb.create_bar((self.tuner_params.max_step) as u64);
 			    pb.show_message = true;
		        
		       
		        				
				let (mut master_state_c, mut problem_c) = (initial_state.clone(), problem.clone());
	        	let (elapsed_steps_c,
	        		initial_states_pool_c,
            		threads_res_c) = 	(elapsed_steps.clone(),
            		 					initial_states_pool.clone(),
    		 							threads_res.clone());

				let nrg_type = self.clone().tuner_params.energy;
				let max_steps= self.clone().tuner_params.max_step;
				let cooling_sched= self.clone().tuner_params.cooling;
				let max_temp=self.tuner_params.max_temp.unwrap().clone();
				let cooler_c=cooler.clone();
				let is=initial_state.clone();
				let mut res_emitter=self.clone().res_emitter;

 	 			/************************************************************************************************************/
 				thread::spawn(move || {
				 	
			        let mut attempted = 0;
			        let mut total_improves = 0;
			        let mut subsequent_improves = 0;
					let mut accepted = 0;
			        let mut rejected = 0;
					let mut temperature = common::Temperature::new(max_temp, cooler_c.clone(), cooling_sched);
					let mut worker_elapsed_steps=0;
							
					let mut rng = thread_rng();
        			let mut start_time = time::precise_time_ns(); 
        			
					let mut worker_state=initial_states_pool_c.remove_one().unwrap();
				 	let mut worker_nrg = match problem_c.energy(&worker_state.clone(), worker_nr) {
			            Some(nrg) => nrg,
			            None => panic!("The initial configuration does not allow to calculate the energy"),
			        };
					
					let mut last_nrg=worker_nrg;
					let mut elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
        			let time_2_complete_hrs = ((elapsed_time as f64) * max_steps as f64) / 3600.0;
  					
  					let range = Range::new(0.0, 1.0);
  					
  					let temperature_c=temperature.clone();
  					
					let (tx, rx) = channel::<IntermediateResults>();
			        // Spawn the thread that will take care of writing results into a CSV file
			        thread::spawn(move || loop {
						let mut elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
			            match rx.recv() {
			                Ok(res) => {
			                    res_emitter.send_update(temperature_c.get(),
			                                                elapsed_time,
			                                                0.0,
			                                                res.last_nrg,
			                                                &res.last_state,
			                                                res.best_nrg,
			                                                &res.best_state,
			                                                worker_elapsed_steps,res.tid);
			                }
			                Err(e) => {} 
			            }
			        });
	 				
			 		
			 		/************************************************************************************************************/			    	
			    	
					let threads_res=common::ThreadsResults::new();

		            loop{	            	

						if worker_elapsed_steps > max_steps || rejected>300{
							break;
						}
				
			            elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
						//let time_2_complete_mins=exec_time*(((max_steps/num_workers) - worker_elapsed_steps) as f64) / 60.0;

			            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------------------------------------------"));
			            println!("{} TID[{}] - Completed Steps: {:.2} - Percentage of Completion: {:.2}% - Estimated \
			                      time to Complete: {:.2} Hrs",
			                     Green.paint("[TUNER]"),
			                     worker_nr,
			                     worker_elapsed_steps,
			                     (worker_elapsed_steps as f64 / (cooler_c.max_steps/num_workers) as f64) * 100.0,
			                     elapsed_time);
			            println!("{} Total Accepted Solutions: {:?} - Current Temperature: {:.2} - Elapsed \
			                      Time: {:.2} s",
			                     Green.paint("[TUNER]"),
			                     accepted,
			                     temperature.get(),
			                     elapsed_time);
			            println!("{} Accepted State: {:?}", Green.paint("[TUNER]"), worker_state);
			            println!("{} Accepted Energy: {:.4} - Last Measured Energy: {:.4}",
			                     Green.paint("[TUNER]"),
			                     worker_nrg,
			                     last_nrg);
			            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------------------------------------------"));

		            	pb.message(&format!("TID [{}] - Status - ", worker_nr));
						pb.inc();
						let mut last_state=worker_state.clone();
		            	worker_state = {
        	
				                let next_state = problem_c.new_state(&worker_state,max_steps,worker_elapsed_steps);
				                
								let accepted_state = match problem_c.energy(&next_state.clone(), worker_nr) {
				                    Some(new_energy) => {
            	                        last_nrg = new_energy;

				            			println!("Thread : {:?} - Step: {:?} - State: {:?} - Energy: {:?}",worker_nr,worker_elapsed_steps,next_state,new_energy);
				 
				                        let de = match nrg_type {
				                            EnergyType::throughput => new_energy - worker_nrg,
				                            EnergyType::latency => -(new_energy - worker_nrg), 
				                        }; 
				
				                        if de > 0.0 || range.ind_sample(&mut rng) <= (de / temperature.get()).exp() {
				                            accepted+=1;
				                        	
				                            worker_nrg = new_energy;
				
				                            if de > 0.0 {
					                        	rejected=0;
				                                total_improves = total_improves + 1;
				                                subsequent_improves = subsequent_improves + 1;
				                            }
				
				                           
				                            next_state
				
				                        } else {
				                        	rejected+=1;
				                        		
				                            worker_state
				                        }
				                    }
				                    None => {
				                        println!("{} The current configuration parameters cannot be evaluated. \
				                                  Skip!",
				                                 Green.paint("[TUNER]"));
				                        worker_state
				                    }
				                };
				                
				                accepted_state
				            };
							
							
							let intermediate_res=IntermediateResults{
				            			last_nrg: last_nrg,
				            			last_state:last_state.clone(),
				            			best_nrg: worker_nrg,
				            			best_state: worker_state.clone(),
				            			tid: worker_nr
				            		};
		            		tx.send(intermediate_res);
				            		
							
			            	worker_elapsed_steps+=1;
			            	
							temperature.update(worker_elapsed_steps);	
					}
		            
		            let res=common::MrResult{
		            	energy: worker_nrg,
		            	state: worker_state,
		            };
		            threads_res_c.push(res);	
	            	pb.finish_print(&format!("Child Thread [{}] Terminated the Execution", worker_nr));
            	
	            })

	        }).collect();

        mb.listen();
        // Wait for all threads to complete before start a search in a new set of neighborhoods.
        for h in handles {
            h.join().unwrap();
        }


        /// *********************************************************************************************************

        // Get results of worker threads (each one will put its best evaluated energy) and
        // choose between them which one will be the best one.
        let mut workers_res = threads_res.get_coll();
        let first_elem = workers_res.pop().unwrap();

        let mut best_energy = first_elem.energy;
        let mut best_state = first_elem.state;

        for elem in workers_res.iter() {
            let diff = match self.tuner_params.energy {
                EnergyType::throughput => elem.energy - best_energy,
                EnergyType::latency => -(elem.energy - best_energy), 
            };
            if diff > 0.0 {
                best_energy = elem.clone().energy;
                best_state = elem.clone().state;
            }
        }



        MrResult {
            energy: best_energy,
            state: best_state,
        }
    }
}
