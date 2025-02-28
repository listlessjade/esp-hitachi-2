use std::{mem::MaybeUninit, os::raw::c_float};

use esp_idf_hal::adc::Adc;
use esp_idf_hal::gpio::ADCPin;
use esp_idf_sys::{
    adc_atten_t_ADC_ATTEN_DB_11, adc_oneshot_unit_handle_t, esp, ntc_config_t, ntc_dev_create,
    ntc_dev_delete, ntc_dev_get_adc_handle, ntc_dev_get_temperature, ntc_device_handle_t,
};

use crate::EspResult;

pub struct ThermistorConfig {
    pub b_value: u32,
    pub r25_ohm: u32,
    pub fixed_ohm: u32,
    pub vdd_mv: u32,
    pub circuit_mode: ThermistorCircuitMode,
}

#[repr(C)]
pub enum ThermistorCircuitMode {
    NtcVcc = 0,
    NtcGnd,
}

pub struct Thermistor<P: ADCPin> {
    pin: P,
    ntc: ntc_device_handle_t,
    adc_handle: adc_oneshot_unit_handle_t,
}

unsafe impl<P: ADCPin> Send for Thermistor<P> {}

impl<P: ADCPin> Thermistor<P> {
    pub fn new(pin: P, cfg: ThermistorConfig) -> EspResult<Self> {
        let mut ntc_handle = MaybeUninit::uninit();
        let mut adc_handle = MaybeUninit::uninit();

        let mut config = ntc_config_t {
            circuit_mode: cfg.circuit_mode as u32,
            unit: P::Adc::unit(),
            atten: adc_atten_t_ADC_ATTEN_DB_11,
            channel: pin.adc_channel(),
            b_value: cfg.b_value,
            r25_ohm: cfg.r25_ohm,
            fixed_ohm: cfg.fixed_ohm,
            vdd_mv: cfg.vdd_mv,
        };

        esp!(unsafe {
            ntc_dev_create(
                std::ptr::from_mut(&mut config),
                ntc_handle.as_mut_ptr(),
                adc_handle.as_mut_ptr(),
            )
        })?;

        let ntc = unsafe { ntc_handle.assume_init() };
        esp!(unsafe { ntc_dev_get_adc_handle(ntc, adc_handle.as_mut_ptr()) })?;

        Ok(Thermistor {
            pin,
            adc_handle: unsafe { adc_handle.assume_init() },
            ntc,
        })
    }

    pub fn get_temp(&mut self) -> EspResult<c_float> {
        let mut temp = 0.0f32;
        esp!(unsafe { ntc_dev_get_temperature(self.ntc, std::ptr::from_mut(&mut temp)) })?;

        Ok(temp)
    }
}

impl<P: ADCPin> Drop for Thermistor<P> {
    fn drop(&mut self) {
        let _ = esp!(unsafe { ntc_dev_delete(self.ntc) });
        // let _ = adc_oneshot
    }
}
