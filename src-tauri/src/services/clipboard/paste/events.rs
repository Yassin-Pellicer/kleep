use std::fs::File;
use std::io::Write;

const EV_KEY: u16 = 1;
const EV_SYN: u16 = 0;
const SYN_REPORT: u16 = 0;
const KEY_LEFTCTRL: u16 = 29;
const KEY_V: u16 = 47;

fn write_event(file: &mut File, event_type: u16, code: u16, value: i32) -> Result<(), String> {
    let event = libc::input_event {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        type_: event_type,
        code,
        value,
    };

    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&event as *const libc::input_event).cast::<u8>(),
            std::mem::size_of::<libc::input_event>(),
        )
    };

    file.write_all(bytes).map_err(|error| error.to_string())
}

pub(super) fn send_paste_shortcut(file: &mut File) -> Result<(), String> {
    // Each key change needs its own SYN_REPORT, otherwise the compositor
    // receives Ctrl and V pressed and released in a single report.
    let press = |file: &mut File, code: u16, value: i32| -> Result<(), String> {
        write_event(file, EV_KEY, code, value)?;
        write_event(file, EV_SYN, SYN_REPORT, 0)?;
        file.flush().map_err(|error| error.to_string())
    };

    press(file, KEY_LEFTCTRL, 1)?;
    press(file, KEY_V, 1)?;
    press(file, KEY_V, 0)?;
    press(file, KEY_LEFTCTRL, 0)?;
    Ok(())
}
