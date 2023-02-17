use crossbeam_channel::Sender;
use evdev::{Device, EventType};
use tokio_stream::{StreamExt, StreamMap};
use tracing::{error, info};

pub enum InputEvent {
    Keyboard,
    Mouse,
}

pub fn start_listener(event_tx: Sender<InputEvent>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        if let Err(e) = run(event_tx).await {
            error!("Failed to run event listener: {e}");
        }
    });
}

#[derive(Debug, thiserror::Error)]
enum RunError {
    #[error("No deivces found! Make sure you are running as root user.")]
    NoDevices,
    #[error("")]
    Std(#[from] std::io::Error),
}

async fn run(event_tx: Sender<InputEvent>) -> Result<(), RunError> {
    let devices: Vec<Device> = evdev::enumerate()
        .map(|(_, device)| device)
        .filter(|d| {
            // Filter on keyboard, mouse & touchscreen devices
            let supported = d.supported_events();
            supported.contains(EventType::KEY)
                || supported.contains(EventType::RELATIVE)
                || supported.contains(EventType::ABSOLUTE)
        })
        .collect();
    if devices.is_empty() {
        return Err(RunError::NoDevices);
    }
    info!("Listening for events on {} input devices", devices.len());
    let mut streams = StreamMap::new();
    for (n, device) in devices.into_iter().enumerate() {
        streams.insert(n, device.into_event_stream()?);
    }
    while let Some((_, Ok(event))) = streams.next().await {
        let event = match event.event_type() {
            EventType::KEY => Some(InputEvent::Keyboard),
            EventType::RELATIVE | EventType::ABSOLUTE => Some(InputEvent::Mouse),
            _ => None,
        };
        if let Some(event) = event {
            if let Err(e) = event_tx.try_send(event) {
                if e.is_disconnected() {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
