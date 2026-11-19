use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, WindowClass,
};

pub(super) const MAX_TEXT_BYTES: usize = 100_000;

pub(super) fn read_clipboard() -> Option<String> {
    // Use XWayland for polling. wl-paste's Wayland fallback has to briefly
    // focus a surface to read the clipboard, which causes visible flicker.
    read_clipboard_x11()
}

fn read_clipboard_x11() -> Option<String> {
    let (connection, screen_num) = x11rb::connect(None).ok()?;
    let screen = &connection.setup().roots[screen_num];
    let window = connection.generate_id().ok()?;

    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new()
                .event_mask(EventMask::PROPERTY_CHANGE)
                .override_redirect(1u32),
        )
        .ok()?;
    connection.map_window(window).ok()?;
    connection.flush().ok()?;

    let clipboard = connection
        .intern_atom(false, b"CLIPBOARD")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let utf8 = connection
        .intern_atom(false, b"UTF8_STRING")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let property = connection
        .intern_atom(false, b"KLEEP_CLIPBOARD")
        .ok()?
        .reply()
        .ok()?
        .atom;

    connection
        .convert_selection(window, clipboard, utf8, property, x11rb::CURRENT_TIME)
        .ok()?;
    connection.flush().ok()?;

    let deadline = std::time::Instant::now() + Duration::from_millis(250);
    let mut result = None;

    while std::time::Instant::now() < deadline {
        match connection.poll_for_event().ok()? {
            Some(Event::SelectionNotify(event)) => {
                if event.property != x11rb::NONE {
                    let reply = connection
                        .get_property(
                            true,
                            window,
                            property,
                            AtomEnum::ANY,
                            0,
                            MAX_TEXT_BYTES as u32 / 4,
                        )
                        .ok()?
                        .reply()
                        .ok()?;

                    if !reply.value.is_empty() {
                        result = Some(String::from_utf8_lossy(&reply.value).to_string());
                    }
                }
                break;
            }
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }

    let _ = connection.destroy_window(window);
    result.filter(|text| !text.is_empty())
}
