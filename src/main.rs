//! Reads from ADC1 and stores the result in a buffer using DMA.
//! Then, does various operations on the buffer!
#![no_main]
#![no_std]


// Import section - load in core stuff for our microcontroller, FPU, and logging.
use core::{mem, mem::MaybeUninit};
use log::info;
use micromath::F32Ext;

use cortex_m_rt::entry;

use stm32h7xx_hal::dma::{
    config::BurstMode,
    dma::{DmaConfig, StreamsTuple},
    Transfer,
};
use stm32h7xx_hal::{adc, delay::Delay, pac, prelude::*};

#[macro_use]
mod utilities;

const SIZE: usize = 1024;

/// Normalize the contents of an array in place
/// This produces a mere 20 instructions, despite using very high-level FP semantics
/// https://godbolt.org/z/vG9cb5ofG
fn normalize_slice(slice: &mut [f32]) -> f32 {
    let sum = slice.iter().fold(0.0, |acc, f| acc + *f);
    let mean = sum / slice.len() as f32;

    slice.iter_mut().for_each(|f| *f -= mean);

    mean
}

#[entry]
fn main() -> ! {
    // Start up core systems!
    utilities::logger::init();
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    #[link_section = ".axisram"]
    static mut BUFFER: MaybeUninit<[u16; SIZE]> = MaybeUninit::uninit();

    // Shenanigans to link in a buffer from the .axisram section
    let adc_buffer: &'static mut [u16; SIZE] = {
        // Convert an uninitialised array into an array of uninitialised
        let buf: &mut [MaybeUninit<u16>; SIZE] = unsafe { mem::transmute(&mut BUFFER) };
        // Initialise memory to valid values
        for slot in buf.iter_mut() {
            // Never create even a _temporary_ reference to uninitialised memory
            unsafe {
                slot.as_mut_ptr().write(0);
            }
        }
        unsafe { mem::transmute(buf) }
    };

    // Constrain and Freeze power
    info!("Setup PWR...                  ");
    let pwr = dp.PWR.constrain();
    let pwrcfg = example_power!(pwr).freeze();

    // Constrain and Freeze clock
    // Fun fact: if you do this wrong, it won't let you compile!
    info!("Setup RCC...                  ");
    let rcc = dp.RCC.constrain();

    let ccdr = rcc
        .sys_ck(96.MHz())
        .pll2_p_ck(4096.kHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    let mut delay = Delay::new(cp.SYST, ccdr.clocks);

    // Setup ADC
    let mut adc1 = adc::Adc::adc1(
        dp.ADC1,
        6114.kHz(),
        &mut delay,
        ccdr.peripheral.ADC12,
        &ccdr.clocks,
    )
    .enable();
    adc1.set_resolution(adc::Resolution::SixteenBit);

    // Setup GPIOC
    let gpioc = dp.GPIOA.split(ccdr.peripheral.GPIOA);

    // Configure pc0 as an analog input
    let mut channel = gpioc.pa3.into_analog(); // ANALOG IN 10

    // 4-beat bursts can be used.
    let config = DmaConfig::default()
        .memory_increment(true)
        .peripheral_burst(BurstMode::Burst4);

    // Setup the DMA transfer on stream 0
    let streams = StreamsTuple::new(dp.DMA1, ccdr.peripheral.DMA1);
    let mut transfer: Transfer<_, _, _, _, _> =
        Transfer::init(streams.0, adc1, &mut adc_buffer[..], None, config);

    info!("About to start transfer...    ");
    
    transfer.start(|adc| {
        // This closure runs right after enabling the stream

        // Start a one-shot conversion for the length of this transfer
        adc.start_conversion_dma(&mut channel, adc::AdcDmaMode::OneShot);
    });

    let target_buffer: &'static mut [u16; SIZE] = unsafe { mem::transmute(&mut BUFFER) };

    // Wait for transfer to complete
    while !transfer.get_transfer_complete_flag() {}

    // Calculate the average of the buffer, into a u32 so we don't overflow
    let sum = target_buffer
        .iter()
        .fold(0u32, |acc, f| acc + *f as u32);

    // Extract these u32 into floats so we can take the fft
    let mut samples: [f32; SIZE] = [0.0; SIZE];
    for (i, value) in target_buffer.iter().enumerate() {
        // Two paths - either copy and convet normally or set some to zero, depending on the part of the lab
        samples[i] = *value as f32;

        // if i < SIZE - (124) {
        //     samples[i] = *value as f32;
        // } else {
        //     samples[i] = 0.0;
        // }
    }

    // Optionally, pull out the first 256 samples (for the lab)
    // let mut buffer: [f32; 256] = samples[0..256].try_into().unwrap();

    // Normalize the samples to remove dc offset
    let mean = normalize_slice(&mut samples);

    info!("Average: {}", mean);

    // Get the FFT using microfft
    let spectrum = microfft::real::rfft_1024(&mut samples);

    // Print the FFT magnitudes
    for (i, value) in spectrum.iter().enumerate() {
        // Fun fact: this print is very cheap due to deferred formatting! Give it a look!
        info!("{i},{}", value.norm_sqr().sqrt());
        delay.delay_us(100_u32);
    }

    loop {
        delay.delay_ms(1000_u32);

        cortex_m::asm::wfi();
    }
}
