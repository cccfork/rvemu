//! The emulator module represents an entire computer.

use crate::cpu::Cpu;
use crate::exception::Trap;

/// The emulator to hold a CPU.
pub struct Emulator {
    /// The CPU which is the core implementation of this emulator.
    pub cpu: Cpu,
    /// The debug flag. Output messages if it's true, otherwise output nothing.
    pub is_debug: bool,
    /// The test flag for riscv/riscv-tests.
    pub is_test: bool,
}

impl Emulator {
    /// Constructor for an emulator.
    pub fn new() -> Emulator {
        Self {
            cpu: Cpu::new(),
            is_debug: false,
            is_test: false,
        }
    }

    /// Reset CPU state.
    pub fn reset(&mut self) {
        self.cpu.reset()
    }

    /// Set binary data to the beginning of the DRAM from the emulator console.
    pub fn set_dram(&mut self, data: Vec<u8>) {
        self.cpu.bus.set_dram(data);
    }

    /// Set binary data to the virtio disk from the emulator console.
    pub fn set_disk(&mut self, data: Vec<u8>) {
        self.cpu.bus.set_disk(data);
    }

    /// Set the program counter to the CPU field.
    pub fn set_pc(&mut self, pc: u64) {
        self.cpu.pc = pc;
    }

    /// Start executing the emulator.
    pub fn start(&mut self) {
        let mut count = 0;
        loop {
            // This is for unit tests to finish the execution.
            count += 1;
            if self.is_test && count < 10000 {
                return;
            }

            // Take an interrupt.
            match self.cpu.check_pending_interrupt() {
                Some(interrupt) => interrupt.take_trap(&mut self.cpu),
                None => {}
            }

            // Increment a CPU timer for a timer interrupt.
            self.cpu.timer_increment();

            // Increment a CPU clock. In one cycle, CPU does fetch, decode, and execute.
            let trap = match self.cpu.tick() {
                Ok(inst) => {
                    if self.is_debug {
                        dbg!(format!(
                            "pc: {:#x} , inst: {:#x}, is_inst 16? {}",
                            self.cpu.pc,
                            inst,
                            // Check if an instruction is one of the compressed instructions.
                            (inst & 0xffff_0000) == 0,
                        ));
                    }
                    // Return a dummy trap.
                    Trap::Requested
                }
                Err(exception) => exception.take_trap(&mut self.cpu),
            };

            match trap {
                Trap::Fatal => {
                    println!("pc: {:#x}, trap {:#?}", self.cpu.pc, trap);
                    return;
                }
                _ => {}
            }
        }
    }
}
