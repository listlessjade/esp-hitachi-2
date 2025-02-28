use std::ffi::c_void;
use std::ptr;
use std::{mem::MaybeUninit, time::Duration};

use arrayvec::ArrayVec;
use esp_idf_hal::gpio::{AnyInputPin, InputPin, Pin};
use esp_idf_sys::{
    button_cb_t, button_config_t, button_dev_t, button_event_args_t, button_gpio_config_t,
    button_handle_t, iot_button_get_event, iot_button_new_gpio_device, iot_button_register_cb,
};
use esp_idf_sys::{esp, EspError};
use thingbuf::mpsc::blocking::StaticSender;

use crate::event_queue::Event;

pub struct ButtonConfig {
    pub long_press_time: Option<Duration>,
    pub short_press_time: Option<Duration>,
    pub active_level: ActiveLevel,
    pub enable_power_save: bool,
    pub disable_pull: bool,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            long_press_time: None,
            short_press_time: None,
            active_level: ActiveLevel::High,
            enable_power_save: false,
            disable_pull: false,
        }
    }
}

pub struct RawButton {
    handle: button_handle_t,
    #[allow(dead_code)]
    pin: i32,
}

impl RawButton {
    pub fn new(pin: i32, cfg: ButtonConfig) -> Result<Self, EspError> {
        let mut handle: MaybeUninit<button_handle_t> = MaybeUninit::uninit();
        let main_cfg = button_config_t {
            long_press_time: cfg
                .long_press_time
                .and_then(|v| v.as_millis().try_into().ok())
                .unwrap_or(0),
            short_press_time: cfg
                .short_press_time
                .and_then(|v| v.as_millis().try_into().ok())
                .unwrap_or(0),
        };
        let gpio_cfg = button_gpio_config_t {
            gpio_num: pin,
            active_level: cfg.active_level as u8,
            enable_power_save: cfg.enable_power_save,
            disable_pull: cfg.disable_pull,
        };

        esp!(unsafe {
            iot_button_new_gpio_device(
                ptr::from_ref(&main_cfg),
                ptr::from_ref(&gpio_cfg),
                handle.as_mut_ptr(),
            )
        })?;

        let handle = unsafe { handle.assume_init() };

        Ok(Self { handle, pin })
    }

    pub fn register_callback(
        &mut self,
        event: ButtonEvent,
        callback: button_cb_t,
        args: *mut button_event_args_t,
        usr_data: *mut c_void,
    ) -> Result<(), EspError> {
        esp!(unsafe {
            iot_button_register_cb(self.handle, event as u32, args, callback, usr_data)
        })?;

        Ok(())
    }
}

#[repr(u8)]
pub enum ActiveLevel {
    Low = 0,
    High = 1,
}

#[repr(u32)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ButtonEvent {
    #[default]
    PressDown = 0,
    PressUp,
    PressRepeat,
    PressRepeatDone,
    SingleClick,
    DoubleClick,
    MultipleClick,
    LongPressStart,
    LongPressHold,
    LongPressUp,
    PressEnd,
    EventMax,
    NonePress,
}

unsafe extern "C" fn btn_callback(arg: *mut c_void, usr_data: *mut c_void) {
    let event = iot_button_get_event(arg as *mut button_dev_t);
    let button_handle = usr_data as *mut ButtonHandlerData;

    if let Some(btn) = button_handle.as_ref() {
        let _ = btn.sender.try_send(Event::Button(
            unsafe { std::mem::transmute(event) },
            btn.pin.pin(),
        ));
    };
}

struct ButtonHandlerData {
    pin: AnyInputPin,
    sender: StaticSender<Event>,
}

pub struct ButtonManager {
    sender: StaticSender<Event>,
    button_data: ArrayVec<ButtonHandlerData, 8>,
    button_handlers: ArrayVec<RawButton, 8>,
}

impl ButtonManager {
    pub fn new(tx: StaticSender<Event>) -> Self {
        ButtonManager {
            sender: tx,
            button_data: ArrayVec::new(),
            button_handlers: ArrayVec::new(),
        }
        // ButtonManager {
        //     sender:
        // }
    }

    pub fn add_button<P: InputPin>(&mut self, pin: P, cfg: ButtonConfig) -> Result<(), EspError> {
        let pin = pin.downgrade_input();
        let mut button = RawButton::new(pin.pin(), cfg)?;

        let managed = ButtonHandlerData {
            pin,
            sender: self.sender.clone(),
        };

        let idx = self.button_data.len();
        self.button_data.push(managed);

        let data_ptr = ptr::from_mut(&mut self.button_data[idx]);

        button.register_callback(
            ButtonEvent::SingleClick,
            Some(btn_callback),
            ptr::null_mut(),
            data_ptr as *mut c_void,
        )?;

        button.register_callback(
            ButtonEvent::DoubleClick,
            Some(btn_callback),
            ptr::null_mut(),
            data_ptr as *mut c_void,
        )?;

        self.button_handlers.push(button);

        Ok(())
    }
}
