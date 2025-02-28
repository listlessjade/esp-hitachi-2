use thingbuf::mpsc::blocking::{StaticChannel, StaticReceiver, StaticSender};

use crate::{ble::LovenseMessage, idf_libs::button::ButtonEvent};

static EVENT_QUEUE: StaticChannel<Event, 64> = StaticChannel::new();

#[derive(Clone, Copy, Default, Debug)]
pub enum Event {
    Lovense(LovenseMessage),
    SetPwm(u8),
    Button(ButtonEvent, i32),
    #[default]
    Null,
}

impl From<(ButtonEvent, i32)> for Event {
    fn from(value: (ButtonEvent, i32)) -> Self {
        Event::Button(value.0, value.1)
    }
}

pub fn get_channel() -> (StaticSender<Event>, StaticReceiver<Event>) {
    EVENT_QUEUE.split()
}
