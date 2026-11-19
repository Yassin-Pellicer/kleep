use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::time::Duration;

const EV_KEY: u16 = 1;
const KEY_LEFTCTRL: u16 = 29;
const KEY_V: u16 = 47;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_INSERT: u16 = 110;
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x0000_5501;

#[repr(C)]
struct UinputInputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

// Matches `struct uinput_setup` from <linux/uinput.h>.
#[repr(C)]
struct UinputSetup {
    id: UinputInputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

/// Creates a virtual keyboard for pasting into the previously focused app.
pub(super) fn create() -> Result<File, String> {
    let file = OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .map_err(|error| format!("direct paste unavailable: {error}"))?;

    let fd = file.as_raw_fd();

    unsafe {
        if libc::ioctl(fd, UI_SET_EVBIT, EV_KEY as libc::c_int) < 0 {
            return Err("could not configure virtual keyboard".to_string());
        }

        for key in [KEY_LEFTCTRL, KEY_V, KEY_LEFTSHIFT, KEY_INSERT] {
            if libc::ioctl(fd, UI_SET_KEYBIT, key as libc::c_int) < 0 {
                return Err("could not configure virtual keyboard keys".to_string());
            }
        }

        // The kernel rejects UI_DEV_CREATE with EINVAL unless the device
        // has been given an identity first.
        let mut setup = UinputSetup {
            id: UinputInputId {
                bustype: 0x03,
                vendor: 0x1234,
                product: 0x5678,
                version: 1,
            },
            name: [0; 80],
            ff_effects_max: 0,
        };
        let name = b"kleep-paste";
        setup.name[..name.len()].copy_from_slice(name);

        if libc::ioctl(fd, UI_DEV_SETUP, &setup as *const UinputSetup) < 0 {
            return Err("could not name virtual keyboard".to_string());
        }

        if libc::ioctl(fd, UI_DEV_CREATE) < 0 {
            return Err("could not create virtual keyboard".to_string());
        }
    }

    // Give the compositor a moment to notice the new input device.
    std::thread::sleep(Duration::from_millis(150));
    Ok(file)
}
