use defmt::{Debug2Format, info};
use sgp40::VocAlgorithmState;

use crate::VOC_ALGO_STATE;

pub const STATE_SIZE: usize = size_of::<VocAlgorithmState>();

pub fn store_voc_state(state: &VocAlgorithmState) {
    info!("Storing: {}", Debug2Format(&state));
    let mut buf = [0u8; STATE_SIZE];
    let mut i = 0;

    macro_rules! put_f32 {
        ($v:expr) => {{
            buf[i..i + 4].copy_from_slice(&$v.to_le_bytes());
            i += 4;
        }};
    }

    put_f32!(state.uptime);
    put_f32!(state.sraw);
    put_f32!(state.gas_index);

    put_f32!(state.mean);
    put_f32!(state.std);
    put_f32!(state.sraw_offset);
    put_f32!(state.uptime_gamma);
    put_f32!(state.uptime_gating);
    put_f32!(state.gating_duration_minutes);

    buf[i] = state.lp_initialized as u8;
    i += 1;

    put_f32!(state.lp_x1);
    put_f32!(state.lp_x2);
    put_f32!(state.lp_x3);
    unsafe { VOC_ALGO_STATE = buf }
}

pub fn restore_voc_state() -> VocAlgorithmState {
    let buf = unsafe { VOC_ALGO_STATE };
    let mut i = 0;

    macro_rules! get_f32 {
        () => {{
            let v = f32::from_le_bytes(buf[i..i + 4].try_into().unwrap());
            i += 4;
            v
        }};
    }

    let uptime = get_f32!();
    let sraw = get_f32!();
    let gas_index = get_f32!();

    let mean = get_f32!();
    let std = get_f32!();
    let sraw_offset = get_f32!();
    let uptime_gamma = get_f32!();
    let uptime_gating = get_f32!();
    let gating_duration_minutes = get_f32!();

    let lp_initialized = buf[i] != 0;
    i += 1;

    let lp_x1 = get_f32!();
    let lp_x2 = get_f32!();
    let lp_x3 = get_f32!();

    VocAlgorithmState {
        uptime,
        sraw,
        gas_index,
        mean,
        std,
        sraw_offset,
        uptime_gamma,
        uptime_gating,
        gating_duration_minutes,
        lp_initialized,
        lp_x1,
        lp_x2,
        lp_x3,
    }
}
