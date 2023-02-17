/// Uses the x11 record API to listen for input events
/// Based on: https://github.com/psychon/x11rb/blob/069be3ace7081705ac6d090347d231447b8276ba/x11rb/examples/record.rs
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::convert::TryFrom;
use std::thread;
use tracing::{error, info};
use x11rb::connect;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::errors::ParseError;
use x11rb::protocol::record::ConnectionExt;
use x11rb::protocol::{record, xproto};
use x11rb::x11_utils::TryParse;

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    KeyDown(u8),
    KeyUp(u8),
    MouseDown(u8),
    MouseUp(u8),
    MouseMove { x: i16, y: i16 },
}

pub fn start_listener() -> Receiver<InputEvent> {
    let (tx, rx) = unbounded();
    thread::spawn(move || match run(tx) {
        Ok(()) => info!("Stopped input listener"),
        Err(e) => {
            error!("Failed to open run input listener: {e}");
        }
    });
    rx
}

fn run(event_tx: Sender<InputEvent>) -> Result<(), Box<dyn std::error::Error>> {
    let (ctrl_conn, _) = connect(None)?;
    let (data_conn, _) = connect(None)?;

    if ctrl_conn
        .extension_information(record::X11_EXTENSION_NAME)?
        .is_none()
    {
        error!("RECORD extension unsupported by X11 server");
        return Ok(());
    }

    let rc = ctrl_conn.generate_id()?;
    let empty = record::Range8 { first: 0, last: 0 };
    let empty_ext = record::ExtRange {
        major: empty,
        minor: record::Range16 { first: 0, last: 0 },
    };
    let range = record::Range {
        core_requests: empty,
        core_replies: empty,
        ext_requests: empty_ext,
        ext_replies: empty_ext,
        delivered_events: empty,
        device_events: record::Range8 {
            first: xproto::KEY_PRESS_EVENT,
            last: xproto::MOTION_NOTIFY_EVENT,
        },
        errors: empty,
        client_started: false,
        client_died: false,
    };

    ctrl_conn
        .record_create_context(rc, 0, &[record::CS::ALL_CLIENTS.into()], &[range])?
        .check()?;

    const RECORD_FROM_SERVER: u8 = 0;
    for reply in data_conn.record_enable_context(rc)? {
        let reply = reply?;
        if reply.category == RECORD_FROM_SERVER {
            let mut remaining = &reply.data[..];
            while !remaining.is_empty() {
                let (r, event) = parse_event(&reply.data)?;
                if let Some(event) = event {
                    if let Err(e) = event_tx.try_send(event) {
                        if e.is_disconnected() {
                            return Ok(());
                        }
                    }
                }
                remaining = r;
            }
        }
    }
    Ok(())
}

fn parse_event(data: &[u8]) -> Result<(&[u8], Option<InputEvent>), ParseError> {
    match data[0] {
        xproto::KEY_PRESS_EVENT => {
            let (event, remaining) = xproto::KeyPressEvent::try_parse(data)?;
            Ok((remaining, Some(InputEvent::KeyDown(event.detail))))
        }
        xproto::KEY_RELEASE_EVENT => {
            let (event, remaining) = xproto::KeyReleaseEvent::try_parse(data)?;
            Ok((remaining, Some(InputEvent::KeyUp(event.detail))))
        }
        xproto::BUTTON_PRESS_EVENT => {
            let (event, remaining) = xproto::ButtonPressEvent::try_parse(data)?;
            Ok((remaining, Some(InputEvent::MouseDown(event.detail))))
        }
        xproto::BUTTON_RELEASE_EVENT => {
            let (event, remaining) = xproto::ButtonReleaseEvent::try_parse(data)?;
            Ok((remaining, Some(InputEvent::MouseUp(event.detail))))
        }
        xproto::MOTION_NOTIFY_EVENT => {
            let (event, remaining) = xproto::MotionNotifyEvent::try_parse(data)?;
            Ok((
                remaining,
                Some(InputEvent::MouseMove {
                    x: event.root_x,
                    y: event.root_y,
                }),
            ))
        }
        0 => {
            let (length, _) = u32::try_parse(&data[4..])?;
            let length = usize::try_from(length).unwrap() * 4 + 32;
            Ok((&data[length..], None))
        }
        _ => Ok((&data[32..], None)),
    }
}
