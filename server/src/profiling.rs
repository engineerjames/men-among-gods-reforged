/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Profiling module - CPU cycle counting and performance profiling

use std::time::{Duration, Instant};

use crate::constants::*;
use crate::logging::Logger;
use crate::types::*;
use crate::xlog;

// Platform-specific rdtsc implementation
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
fn rdtsc_impl() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
fn rdtsc_impl() -> u64 {
    // Fallback: use high-resolution timer
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_nanos() as u64
}

/// Profile task names corresponding to original C++ code
pub static PROFNAME: [&str; 46] = [
    "misc",                 // 0
    "  pathfinder",         // 1
    "  area_log",           // 2
    "  area_say",           // 3
    "  area_sound",         // 4
    "  area_notify",        // 5
    "  npc_shout",          // 6
    "  update_char",        // 7
    " regenerate",          // 8
    "  add_light",          // 9
    " getmap",              // 10
    " change",              // 11
    " act",                 // 12
    " pop_tick",            // 13
    " effect_tick",         // 14
    " item_tick",           // 15
    "   can_see",           // 16
    "   can_go",            // 17
    "   compute_dlight",    // 18
    "   remove_lights",     // 19
    "   add_lights",        // 20
    "   char_can_see",      // 21
    "   char_can_see_item", // 22
    "   god_create_item",   // 23
    " driver",              // 24
    "tick",                 // 25
    "global_tick",          // 26
    " npc_driver",          // 27
    "  drv_dropto",         // 28
    "  drv_pickup",         // 29
    "  drv_give",           // 30
    "  drv_use",            // 31
    "  drv_bow",            // 32
    "  drv_wave",           // 33
    "  drv_turn",           // 34
    "  drv_attack",         // 35
    "  drv_moveto",         // 36
    "  drv_skill",          // 37
    "  drv_use",            // 38
    "  npc_high",           // 39
    "  plr_driver_med",     // 40
    " ccp_driver",          // 41
    "net IO",               // 42
    "IDLE",                 // 43
    "compress",             // 44
    " plr_save_char",       // 45
];

/// Profiler for tracking CPU cycles spent in different parts of the code
pub struct Profiler {
    /// Profile table storing cycles per task
    pub proftab: [u64; 100],
    /// Estimated CPU cycles per second
    pub cycles_per_sec: f64,
    /// Tick counter for profiling interval
    pub tick_counter: i32,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            proftab: [0; 100],
            cycles_per_sec: 0.0,
            tick_counter: 0,
        }
    }
    
    /// Calibrate the profiler by measuring CPU cycles over 1 second
    pub fn calibrate(&mut self) {
        // Speed test: measure cycles over 1 second
        let t1 = self.rdtsc();
        std::thread::sleep(Duration::from_secs(1));
        let t2 = self.rdtsc();
        
        let diff = t2 - t1;
        // Round to nearest 50 MHz
        self.cycles_per_sec = ((diff + 25_000_000) / 50_000_000 * 50_000_000) as f64;
    }
    
    /// Get the current timestamp counter value (rdtsc)
    #[inline]
    pub fn rdtsc(&self) -> u64 {
        rdtsc_impl()
    }
    
    /// Start profiling - returns the current cycle count
    #[inline]
    pub fn prof_start(&self) -> u64 {
        self.rdtsc()
    }
    
    /// Stop profiling and record the elapsed cycles
    #[inline]
    pub fn prof_stop(&mut self, task: usize, start_cycle: u64) {
        let td = self.rdtsc() - start_cycle;
        
        if task < 100 {
            self.proftab[task] += td;
        }
    }
    
    /// Display profiling results to characters with CF_PROF flag
    pub fn god_prof(&self, ch: &[Character], logger: &Logger, do_char_log: impl Fn(usize, i32, &str)) {
        let mut bestn = [-1i32; MAX_BEST];
        let mut bestv = [1u64; MAX_BEST];
        
        // Find the top MAX_BEST tasks by cycle count
        for n in 0..100 {
            for m in 0..MAX_BEST {
                if self.proftab[n] > bestv[m] {
                    if m < MAX_BEST - 1 {
                        // Shift existing entries down
                        for i in (m + 1..MAX_BEST).rev() {
                            bestv[i] = bestv[i - 1];
                            bestn[i] = bestn[i - 1];
                        }
                    }
                    bestv[m] = self.proftab[n];
                    bestn[m] = n as i32;
                    break;
                }
            }
        }
        
        // Display to all characters with CF_PROF flag
        for cn in 1..MAXCHARS {
            if ch[cn].used == USE_EMPTY {
                continue;
            }
            if !ch[cn].has_prof() {
                continue;
            }
            
            for n in 0..MAX_BEST {
                if bestn[n] == -1 {
                    break;
                }
                
                let task_idx = bestn[n] as usize;
                let font = if PROFNAME.get(task_idx).map_or(false, |&name| name == "IDLE") {
                    3
                } else {
                    2
                };
                
                let name = PROFNAME.get(task_idx).unwrap_or(&"unknown");
                let percentage = 100.0 / (self.cycles_per_sec * PROF_FREQ as f64) 
                               * self.proftab[task_idx] as f64;
                
                do_char_log(cn, font, &format!("{:20.20} {:3.2}%\n", name, percentage));
            }
            do_char_log(cn, 2, " \n");
        }
    }
    
    /// Called every tick to potentially output profiling data
    pub fn prof_tick(&mut self, ch: &[Character], logger: &Logger, do_char_log: impl Fn(usize, i32, &str)) {
        self.tick_counter += 1;
        if self.tick_counter < TICKS * PROF_FREQ {
            return;
        }
        self.tick_counter = 0;
        
        self.god_prof(ch, logger, do_char_log);
        
        // Reset profile table
        for n in 0..100 {
            self.proftab[n] = 0;
        }
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}
