/// ///////////////////////////////////////////////////////////////////////////
///  File: annealing/solver/spis.rs
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

/******************************************************************************
*******************************************************************************
/// **
/// Simultaneous Periodically Interacting Searcher (SPIS)
/// *  
*******************************************************************************
*******************************************************************************/
use annealing::solver::Solver;
use annealing::problem::Problem;
use annealing::cooler::{Cooler, StepsCooler, TimeCooler};
use annealing::solver::common;
use annealing::solver::common::MrResult;
use results_emitter;
use results_emitter::{Emitter, Emitter2File};

use time;
use CoolingSchedule; 
use EnergyType;
use hwloc;
use pbr;
use rand;
use libc;
use num_cpus;

use rand::{Rng,thread_rng};
use rand::distributions::{Range, IndependentSample};
use ansi_term::Colour::Green;
use std::collections::HashMap;
use pbr::{ProgressBar,MultiBar};
use std::thread;


#[derive(Debug, Clone)]
pub struct Spis {
	pub min_temp: f64,
	pub max_temp: f64,		
    pub max_steps: usize,
    pub cooling_schedule: CoolingSchedule,
    pub energy_type: EnergyType,
}

impl Solver for Spis {

	fn solve(&mut self, problem: &mut Problem) -> MrResult {
                        	
        	        	
    	let cooler=StepsCooler {
                      max_steps:self.max_steps,
                      min_temp: self.min_temp,
                      max_temp: self.max_temp,
                      };
    	                	
        let mut results_emitter = Emitter2File::new();

        ("{}",Green.paint("\n-------------------------------------------------------------------------------------------------------------------"));
        println!("{} Initialization Phase: Evaluation of Energy for Default Parameters",
                 Green.paint("[TUNER]"));
        println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));

        let mut start_time = time::precise_time_ns();

        let mut master_state = problem.initial_state();
        let mut master_energy = match problem.energy(&master_state.clone(), self.energy_type.clone(),0) {
            Some(nrg) => nrg,
            None => panic!("The initial configuration does not allow to calculate the energy"),
        };

        let mut elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
        let time_2_complete_hrs = ((elapsed_time as f64) * self.max_steps as f64) / 3600.0;

        
		let mut elapsed_steps = common::ElapsedSteps::new();
		let mut accepted = common::AcceptedStates::new();
        let mut rejected = common::SubsequentRejStates::new();
		let mut temperature = common::Temperature::new(self.max_temp, cooler, self.clone().cooling_schedule);
		
        let mut attempted = 0;
        let mut total_improves = 0;
        let mut subsequent_improves = 0;

 		/************************************************************************************************************/
        start_time = time::precise_time_ns();
        'outer: loop {
        	
        		if elapsed_steps.get() > self.max_steps{
        			break 'outer;
        		}
	        	elapsed_time = (time::precise_time_ns() - start_time) as f64 / 1000000000.0f64;
	
	            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));
	            println!("{} Completed Steps: {:.2} - Percentage of Completion: {:.2}% - Estimated \
	                      time to Complete: {:.2} Hrs",
	                     Green.paint("[TUNER]"),
	                     elapsed_steps.get(),
	                     (elapsed_steps.get() as f64 / self.max_steps as f64) * 100.0,
	                     time_2_complete_hrs as usize);
	            println!("{} Total Accepted Solutions: {:?} - Current Temperature: {:.2} - Elapsed \
	                      Time: {:.2} s",
	                     Green.paint("[TUNER]"),
	                     accepted.get(),
	                     temperature.get(),
	                     elapsed_time);
	            println!("{} Accepted State: {:?}", Green.paint("[TUNER]"), master_state);
	            println!("{} Accepted Energy: {:.4}",
	                     Green.paint("[TUNER]"),
	                     master_energy);
	            println!("{}",Green.paint("-------------------------------------------------------------------------------------------------------------------"));
	
				
				//Create the Pool of Neighborhoods
				let neigh_space=problem.neigh_space(&master_state);
				let neigh_pool=common::NeighborhoodsPool::new(neigh_space);
				
				let threads_res=common::ThreadsResults::new();
				
				
		    	
 				let mut mb = MultiBar::new();
 
 				//Get the number of physical cpu cores
			 	let num_cores = common::get_num_cores();	
 				/************************************************************************************************************/
	 			let handles: Vec<_> = (0..num_cores).map(|core| {
	 				let mut pb=mb.create_bar(neigh_pool.size()/num_cores as u64);
 			        pb.show_message = true;
		            					
					let (mut master_state_c, mut problem_c) = (master_state.clone(), problem.clone());
	            	let (elapsed_steps_c, temperature_c,
	            		 neigh_pool_c, accepted_c,
	            		 rejected_c,threads_res_c) = (elapsed_steps.clone(),
	            		 							  temperature.clone(),
	            		 							  neigh_pool.clone(), 
	            		 							  accepted.clone(), 
	            		 							  rejected.clone(),
	            		 							  threads_res.clone());

					let nrg_type = self.clone().energy_type;
					
					
					/************************************************************************************************************/
		            thread::spawn(move || {

							let mut worker_nrg=master_energy;
							let mut worker_state=master_state_c;
  					        let range = Range::new(0.0, 1.0);
		  					let mut rng = thread_rng();


				            loop{
				            	pb.message(&format!("TID [{}] - Neigh. Exploration Status - ", core));

				            	worker_state = {
	            	
						                let next_state = match neigh_pool_c.remove_one(){
							            		Some(res) => res,
							            		None 	  => break,
						            	};

										let accepted_state = match problem_c.energy(&next_state.clone(), nrg_type.clone(), core) {
						                    Some(new_energy) => {
						            			println!("Thread : {:?} - Step: {:?} - State: {:?} - Energy: {:?}",core, elapsed_steps_c.get(),next_state,new_energy);

						                        let de = match nrg_type {
						                            EnergyType::throughput => new_energy - worker_nrg,
						                            EnergyType::latency => -(new_energy - worker_nrg), 
						                        };
						
						                        if de > 0.0 || range.ind_sample(&mut rng) <= (de / temperature_c.get()).exp() {
						                            accepted_c.increment();
						                        	rejected_c.reset();
						                        	
						                            worker_nrg = new_energy;
						
						                           /* if de > 0.0 {
						                                total_improves = total_improves + 1;
						                                subsequent_improves = subsequent_improves + 1;
						                            }*/
						 
						                            /*results_emitter.send_update(new_energy,
						                                                &next_state,
						                                                energy,
						                                                &next_state,
						                                                elapsed_steps_c.get());*/
						                            next_state
						
						                        } else {
						                        	rejected_c.increment();
						                        	
						                        	if rejected_c.get()==50{
						                        		break;
						                        	} 
						                        		
						                           // subsequent_improves = 0;
						                            /*results_emitter.send_update(new_energy,
						                                                &next_state,
						                                                energy,
						                                                &state,
						                                                elapsed_steps_c.get());*/
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
				            	
					            	elapsed_steps_c.increment();
 									pb.inc();	            	
									temperature_c.update(elapsed_steps_c.get());	
							}
				            
				            let res=common::MrResult{
				            	energy: worker_nrg,
				            	state: worker_state,
				            };

				            threads_res_c.push(res);	
    		            	pb.finish_print(&format!("Child Thread [{}] Terminated the Execution", core));
	                	
		            })

		        }).collect();
				
				mb.listen(); 
		        // Wait for all threads to complete before start a search in a new set of neighborhoods.
		        for h in handles {
		            h.join().unwrap();
		        }
		        
		        
				/************************************************************************************************************/	
		        //Get results of worker threads (each one will put its best evaluated energy) and 
		        //choose between them which one will be the best
		        let mut workers_res = threads_res.get_coll();
		       	let first_elem = workers_res.pop().unwrap();
		       	
		       	master_energy = first_elem.energy;
		       	master_state  = first_elem.state;
		       	
		       	for elem in workers_res.iter() {
		       		let diff=match self.energy_type {
                            EnergyType::throughput => {
                            	 elem.energy-master_energy
                            },
                            EnergyType::latency => {
                            	-(elem.energy-master_energy)
                            } 
                        };
		       		if diff > 0.0 {
		       			master_energy=elem.clone().energy;
		       			master_state=elem.clone().state;
		       		}
		       	}
		       
			}

		MrResult {
                  energy: master_energy,
                  state: master_state,
                  }
    } 
}





